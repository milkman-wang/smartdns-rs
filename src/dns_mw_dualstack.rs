use std::net::IpAddr;
use std::time::Duration;

use futures::FutureExt;
use futures::future::{Either, select};
use tokio::time::sleep;

use crate::config::SpeedCheckMode;
use crate::dns::*;
use crate::log::debug;
use crate::middleware::*;
use crate::third_ext::FutureTimeoutExt;

pub struct DnsDualStackIpSelectionMiddleware;

impl DnsDualStackIpSelectionMiddleware {
    pub fn is_configured(cfg: &crate::dns_conf::RuntimeConfig) -> bool {
        cfg.dualstack_ip_selection()
            || cfg.dualstack_ip_prefer_ipv4()
            || cfg.rule_groups().values().any(|group| {
                group
                    .domain_rules
                    .iter()
                    .any(|rule| rule.config.dualstack_ip_selection == Some(true))
            })
    }
}

#[async_trait::async_trait]
impl Middleware<DnsContext, DnsRequest, DnsResponse, DnsError>
    for DnsDualStackIpSelectionMiddleware
{
    async fn handle(
        &self,
        ctx: &mut DnsContext,
        req: &DnsRequest,
        next: Next<'_, DnsContext, DnsRequest, DnsResponse, DnsError>,
    ) -> Result<DnsResponse, DnsError> {
        use RecordType::{A, AAAA};

        // highest priority
        if ctx.server_opts.no_dualstack_selection() {
            return next.run(ctx, req).await;
        }

        let query_type = req.query().query_type();

        // must be ip query.
        if !query_type.is_ip_addr() {
            return next.run(ctx, req).await;
        }

        if ctx.cfg().dualstack_ip_prefer_ipv4() {
            if query_type == A {
                return next.run(ctx, req).await;
            }
            return prefer_ipv4(ctx, req, next).await;
        }

        if ctx.server_opts.no_speed_check() {
            return next.run(ctx, req).await;
        }

        let mut prefer_that = false; // As long as it succeeds, there is no need to check the selection threshold.

        if matches!(query_type, A) {
            if ctx.cfg().dualstack_ip_allow_force_aaaa() {
                prefer_that = true;
            } else {
                return next.run(ctx, req).await;
            }
        }

        // read config
        let dualstack_ip_selection = ctx
            .domain_rule
            .as_ref()
            .map(|rule| rule.dualstack_ip_selection)
            .unwrap_or_default()
            .unwrap_or(ctx.cfg().dualstack_ip_selection());

        if !dualstack_ip_selection {
            return next.run(ctx, req).await;
        }

        let selection_threshold =
            Duration::from_millis(ctx.cfg().dualstack_ip_selection_threshold());

        let speed_check_mode = ctx
            .domain_rule
            .get_ref(|r| r.speed_check_mode.as_ref())
            .or_else(|| ctx.cfg().speed_check_mode())
            .cloned();
        let Some(speed_check_mode) = speed_check_mode
            .filter(|modes| !modes.is_empty() && !modes.iter().any(SpeedCheckMode::is_none))
        else {
            return next.run(ctx, req).await;
        };

        let ttl = ctx.cfg().local_ttl() as u32;

        let that_type = match query_type {
            A => AAAA,
            AAAA => A,
            typ => typ,
        };

        let mut that_ctx = ctx.clone();
        that_ctx.is_dualstack = true;
        let that_req = {
            let mut req = req.clone();
            req.set_query_type(that_type);
            req
        };

        let that_next = next.clone();
        let that = async move {
            let start = std::time::Instant::now();
            let result = that_next.run(&mut that_ctx, &that_req).await;
            crate::stats::completed(&that_ctx, &that_req, &result, start.elapsed());
            crate::plugins::completed(&that_ctx, &that_req, &result, start, None);
            result
        };
        let that = std::pin::pin!(that);
        let this = next.run(ctx, req);

        let dual_task = futures::future::select(this, that).await;

        let this_no_records = || {
            debug!(
                "dual stack IP selection: {} , choose {}",
                req.query().name(),
                that_type
            );
            let query = req.query().original().clone();
            let mut response = DnsResponse::new_with_max_ttl(query.clone(), []);
            response.add_authority(Record::from_rdata(
                query.name().clone(),
                ttl,
                RData::default_soa(),
            ));
            Ok(response)
        };

        match dual_task {
            Either::Left((res, that)) => match res {
                Ok(this) => {
                    let that = that.timeout(selection_threshold).await;

                    if let Ok(Ok(that)) = that {
                        let that_faster = matches!(
                            which_faster(&this, &that, &speed_check_mode, selection_threshold)
                                .await,
                            Either::Right(_)
                        );

                        if that_faster && (prefer_that || matches!(query_type, AAAA)) {
                            return this_no_records();
                        }
                    }

                    Ok(this)
                }
                Err(err) => Err(err),
            },
            Either::Right((res, this)) => match res {
                Ok(that) => match this.await {
                    Ok(this) => {
                        let that_faster = matches!(
                            which_faster(&this, &that, &speed_check_mode, selection_threshold)
                                .await,
                            Either::Right(_)
                        );

                        if that_faster && (prefer_that || matches!(query_type, AAAA)) {
                            return this_no_records();
                        }
                        Ok(this)
                    }
                    Err(err) => Err(err),
                },
                Err(_) => this.await,
            },
        }
    }
}

async fn prefer_ipv4(
    ctx: &mut DnsContext,
    req: &DnsRequest,
    next: Next<'_, DnsContext, DnsRequest, DnsResponse, DnsError>,
) -> Result<DnsResponse, DnsError> {
    let ttl_limit = [
        ctx.domain_rule
            .get_ref(|rule| rule.rr_ttl_max.as_ref().or(rule.rr_ttl.as_ref()))
            .copied()
            .or_else(|| ctx.cfg().rr_ttl_max()),
        ctx.cfg().rr_ttl_reply_max(),
    ]
    .into_iter()
    .flatten()
    .min()
    .unwrap_or(u32::MAX as u64)
    .min(u32::MAX as u64) as u32;
    let modes = if ctx.server_opts.no_speed_check() {
        None
    } else {
        ctx.domain_rule
            .get_ref(|r| r.speed_check_mode.as_ref())
            .or_else(|| ctx.cfg().speed_check_mode())
            .filter(|m| !m.is_empty() && !m.iter().any(SpeedCheckMode::is_none))
            .cloned()
    };
    let mut ipv4_ctx = ctx.clone();
    ipv4_ctx.is_dualstack = true;
    let mut ipv4_req = req.clone();
    ipv4_req.set_query_type(RecordType::A);
    let ipv4_next = next.clone();
    let ipv4 = async move {
        let start = std::time::Instant::now();
        let response = ipv4_next.run(&mut ipv4_ctx, &ipv4_req).await;
        crate::stats::completed(&ipv4_ctx, &ipv4_req, &response, start.elapsed());
        crate::plugins::completed(&ipv4_ctx, &ipv4_req, &response, start, None);
        match response {
            Ok(response) => {
                let usable = ipv4_usable(&response, modes.as_deref().map(Vec::as_slice)).await;
                // Do not retain the preference longer than the A answer supporting it.
                usable.then(|| response.min_ttl().unwrap_or(0).min(ttl_limit))
            }
            Err(_) => None,
        }
    };
    let ipv4 = std::pin::pin!(ipv4);
    let ipv6 = next.run(ctx, req);
    let nodata = |ttl| {
        let query = req.query().original().clone();
        let mut response = DnsResponse::new_with_max_ttl(query.clone(), []);
        response.add_authority(Record::from_rdata(
            query.name().clone(),
            ttl,
            RData::default_soa(),
        ));
        Ok(response)
    };
    match select(ipv4, ipv6).await {
        Either::Left((Some(ttl), _)) => nodata(ttl),
        Either::Left((None, ipv6)) => ipv6.await,
        Either::Right((response, ipv4)) => {
            if let Some(ttl) = ipv4.await {
                nodata(ttl)
            } else {
                response
            }
        }
    }
}

async fn ipv4_usable(response: &DnsResponse, modes: Option<&[SpeedCheckMode]>) -> bool {
    let addresses: Vec<_> = response
        .ip_addrs()
        .into_iter()
        .filter(IpAddr::is_ipv4)
        .collect();
    if addresses.is_empty() {
        return false;
    }
    match response.probe_result() {
        ProbeResult::Measured(_) => true,
        ProbeResult::Failed => false,
        ProbeResult::Unchecked => match modes {
            Some(modes) => multi_mode_ping_fastest(addresses, modes.to_vec())
                .await
                .is_some(),
            // Proxy/no-probe groups rely on a valid A answer, not a direct-path probe.
            None => true,
        },
    }
}

async fn which_faster(
    this: &DnsResponse,
    that: &DnsResponse,
    modes: &[SpeedCheckMode],
    selection_threshold: Duration,
) -> Either<(), ()> {
    match (this.probe_result(), that.probe_result()) {
        (ProbeResult::Measured(this), ProbeResult::Measured(that)) => {
            return if this.saturating_sub(that) > selection_threshold {
                Either::Right(())
            } else {
                Either::Left(())
            };
        }
        (ProbeResult::Failed, ProbeResult::Measured(_)) => return Either::Right(()),
        (ProbeResult::Measured(_), ProbeResult::Failed) => return Either::Left(()),
        _ => (),
    }
    let this_ip_addrs = this.ip_addrs();
    let that_ip_addrs = that.ip_addrs();

    let this_ping = multi_mode_ping_fastest(this_ip_addrs, modes.to_vec()).boxed();
    let that_ping = multi_mode_ping_fastest(that_ip_addrs, modes.to_vec()).boxed();

    let which_faster = select(this_ping, that_ping).await;

    let that_faster = match which_faster {
        Either::Right((Some((_, that_dura)), this_ping)) => match this_ping.await {
            Some((_, this_dura)) => {
                this_dura > that_dura && (this_dura - that_dura) > selection_threshold
            }
            None => true,
        },
        _ => false,
    };

    if that_faster {
        Either::Right(())
    } else {
        Either::Left(())
    }
}

async fn multi_mode_ping_fastest(
    ip_addrs: Vec<IpAddr>,
    modes: Vec<SpeedCheckMode>,
) -> Option<(IpAddr, Duration)> {
    use crate::infra::ping::{PingOptions, ping_fastest};
    let duration = Duration::from_millis(200);
    let ping_ops = PingOptions::default().with_timeout_secs(2);

    let mut fastest_ip = None;

    for mode in &modes {
        let dests = mode.to_ping_addrs(&ip_addrs);

        let ping_task = ping_fastest(dests, ping_ops).boxed();
        let timeout_task = sleep(duration).boxed();
        match futures_util::future::select(ping_task, timeout_task).await {
            futures::future::Either::Left((ping_res, _)) => {
                match ping_res {
                    Ok(ping_out) => {
                        // ping success
                        let ip = ping_out.dest().ip_addr();
                        let duration = ping_out.elapsed();
                        fastest_ip = Some((ip, duration));
                        break;
                    }
                    Err(_) => continue,
                }
            }
            futures::future::Either::Right((_, _)) => {
                // timeout
                continue;
            }
        }
    }

    fastest_ip
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        dns_conf::RuntimeConfig,
        dns_mw::DnsMockMiddleware,
        libdns::proto::op::{Query, ResponseCode},
    };

    #[tokio::test]
    async fn ipv4_preference_preserves_ipv6_fallback_and_a_answers() {
        for (probe, speed, filtered) in [
            (
                Some(ProbeResult::Measured(Duration::from_millis(300))),
                "tcp:443",
                true,
            ),
            (Some(ProbeResult::Failed), "tcp:443", false),
            (None, "tcp:443", false),
            (Some(ProbeResult::Unchecked), "none", true),
            (None, "none", false),
        ] {
            let cfg = RuntimeConfig::builder()
                .with("dualstack-ip-selection no")
                .with("dualstack-ip-prefer-ipv4 yes")
                .with("local-ttl 1")
                .with("rr-ttl-reply-max 60")
                .with("dualstack-ip-allow-force-AAAA yes")
                .with(&format!(
                    "domain-rules /dualstack.example/ -d no -c {speed}"
                ))
                .build()
                .unwrap();
            let a = Query::query("dualstack.example.".parse().unwrap(), RecordType::A);
            let aaaa = Query::query(a.name().clone(), RecordType::AAAA);
            let ipv6 =
                DnsResponse::from_rdata(aaaa.clone(), RData::AAAA("2001:db8::6".parse().unwrap()))
                    .with_probe_result(ProbeResult::Measured(Duration::from_millis(1)));
            let ipv4 = match probe {
                Some(probe) => {
                    DnsResponse::from_rdata(a.clone(), RData::A("192.0.2.4".parse().unwrap()))
                        .with_probe_result(probe)
                }
                None => DnsResponse::new_with_max_ttl(a.clone(), []),
            };
            let expected_a = ipv4.ip_addrs();
            let a_ttl = ipv4.min_ttl().unwrap_or(0);
            let handler = DnsMockMiddleware::mock(DnsDualStackIpSelectionMiddleware)
                .with_result(a.clone(), Ok(ipv4))
                .with_result(aaaa.clone(), Ok(ipv6))
                .build(cfg);
            let response = handler
                .lookup(aaaa.name().clone(), RecordType::AAAA)
                .await
                .unwrap();
            assert_eq!(response.response_code(), ResponseCode::NoError);
            if filtered {
                assert!(response.answers().is_empty());
                assert_eq!(response.authorities()[0].record_type(), RecordType::SOA);
                assert_eq!(response.authorities()[0].ttl(), a_ttl.min(60));
            } else {
                assert_eq!(
                    response.ip_addrs(),
                    vec!["2001:db8::6".parse::<IpAddr>().unwrap()]
                );
            }
            let response = handler
                .lookup(a.name().clone(), RecordType::A)
                .await
                .unwrap();
            assert_eq!(response.ip_addrs(), expected_a);
        }
    }

    #[tokio::test]
    async fn ipv4_lookup_errors_do_not_block_ipv6() {
        let cfg = RuntimeConfig::builder()
            .with("dualstack-ip-prefer-ipv4 yes")
            .build()
            .unwrap();
        let handler = DnsMockMiddleware::mock(DnsDualStackIpSelectionMiddleware)
            .with_aaaa_record("ipv6-only.example", "2001:db8::6".parse().unwrap())
            .build(cfg);
        let response = handler
            .lookup("ipv6-only.example", RecordType::AAAA)
            .await
            .unwrap();
        assert_eq!(
            response.ip_addrs(),
            vec!["2001:db8::6".parse::<IpAddr>().unwrap()]
        );
    }

    #[tokio::test]
    async fn ipv6_lookup_errors_do_not_block_usable_ipv4() {
        let cfg = RuntimeConfig::builder()
            .with("dualstack-ip-prefer-ipv4 yes")
            .build()
            .unwrap();
        let a = Query::query("ipv4-only.example.".parse().unwrap(), RecordType::A);
        let ipv4 = DnsResponse::from_rdata(a.clone(), RData::A("192.0.2.4".parse().unwrap()))
            .with_probe_result(ProbeResult::Measured(Duration::from_millis(1)));
        let handler = DnsMockMiddleware::mock(DnsDualStackIpSelectionMiddleware)
            .with_result(a.clone(), Ok(ipv4))
            .build(cfg);
        let response = handler
            .lookup(a.name().clone(), RecordType::AAAA)
            .await
            .unwrap();
        assert_eq!(response.response_code(), ResponseCode::NoError);
        assert!(response.answers().is_empty());
        assert_eq!(response.authorities()[0].record_type(), RecordType::SOA);
    }

    #[tokio::test]
    async fn unchecked_ipv4_requires_a_successful_configured_probe() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let modes = [SpeedCheckMode::Tcp(listener.local_addr().unwrap().port())];
        let query = Query::query("probe.example.".parse().unwrap(), RecordType::A);
        let response = DnsResponse::from_rdata(query, RData::A("127.0.0.1".parse().unwrap()));
        assert!(ipv4_usable(&response, Some(&modes)).await);
        drop(listener);
        assert!(!ipv4_usable(&response, Some(&modes)).await);
    }
    #[tokio::test]
    async fn test_dualstack_uses_configured_probe_and_returns_nodata() {
        for (speed, filtered) in [("tcp-syn:443", true), ("none", false)] {
            let cfg = RuntimeConfig::builder()
                .with("dualstack-ip-selection yes")
                .with(&format!("speed-check-mode {speed}"))
                .build()
                .unwrap();
            let a = Query::query("dualstack.example.".parse().unwrap(), RecordType::A);
            let aaaa = Query::query(a.name().clone(), RecordType::AAAA);
            let ipv4 = DnsResponse::from_rdata(a.clone(), RData::A("192.0.2.4".parse().unwrap()))
                .with_probe_result(ProbeResult::Measured(Duration::from_millis(1)));
            let ipv6 =
                DnsResponse::from_rdata(aaaa.clone(), RData::AAAA("2001:db8::6".parse().unwrap()))
                    .with_probe_result(ProbeResult::Measured(Duration::from_millis(1000)));
            let handler = DnsMockMiddleware::mock(DnsDualStackIpSelectionMiddleware)
                .with_result(a, Ok(ipv4))
                .with_result(aaaa.clone(), Ok(ipv6))
                .build(cfg);
            let response = handler
                .lookup(aaaa.name().clone(), RecordType::AAAA)
                .await
                .unwrap();
            assert_eq!(response.response_code(), ResponseCode::NoError);
            assert_eq!(response.answers().is_empty(), filtered);
            if filtered {
                assert_eq!(response.authorities()[0].record_type(), RecordType::SOA);
            }
        }
    }
    #[test]
    fn configured_detection_keeps_domain_and_group_overrides() {
        for (lines, expected) in [
            (vec!["dualstack-ip-selection no"], false),
            (vec!["dualstack-ip-selection yes"], true),
            (
                vec!["dualstack-ip-selection no", "dualstack-ip-prefer-ipv4 yes"],
                true,
            ),
            (
                vec![
                    "dualstack-ip-selection no",
                    "domain-rule /example/ -dualstack-ip-selection yes",
                ],
                true,
            ),
            (
                vec![
                    "dualstack-ip-selection no",
                    "domain-rule /example/ -dualstack-ip-selection no",
                ],
                false,
            ),
            (
                vec![
                    "dualstack-ip-selection no",
                    "group-begin work",
                    "domain-rule /example/ -dualstack-ip-selection yes",
                    "group-end",
                ],
                true,
            ),
        ] {
            let mut builder = RuntimeConfig::builder();
            for line in &lines {
                builder = builder.with(line);
            }
            assert_eq!(
                DnsDualStackIpSelectionMiddleware::is_configured(&builder.build().unwrap()),
                expected,
                "{lines:?}"
            );
        }
    }
}
