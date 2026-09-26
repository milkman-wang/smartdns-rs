use crate::log;
use socket2::{Domain, Protocol, SockRef, Socket, Type};
use std::{io, net::SocketAddr};
use tokio::net::{TcpListener, UdpSocket};

pub fn bind_to<T: LocalAddr>(
    new_socket: impl Fn(SocketAddr, Option<&str>) -> io::Result<T>,
    bind_addr: SocketAddr,
    bind_device: Option<&str>,
    bind_type: &str,
) -> T {
    let device_note = bind_device
        .map(|device| format!("@{device}"))
        .unwrap_or_default();

    log::debug!("binding {} to {:?}{}", bind_type, bind_addr, device_note);

    match new_socket(bind_addr, bind_device) {
        Ok(socket) => {
            let local_addr = socket.local_addr().expect("could not lookup local address");
            log::info!(
                "listening for {} on {:?}{}",
                bind_type,
                local_addr,
                device_note
            );
            socket
        }
        Err(err) => panic!("cound not bind to {bind_type}: {bind_addr}, {err}"),
    }
}

pub fn setup_tcp_socket(
    bind_addr: SocketAddr,
    bind_device: Option<&str>,
) -> io::Result<TcpListener> {
    let socket = Socket::new(
        Domain::for_address(bind_addr),
        Type::STREAM,
        Some(Protocol::TCP),
    )?;

    setup_socket(&socket, bind_device, bind_addr)?;

    socket.listen(128)?;

    let tcp_listener = std::net::TcpListener::from(socket);

    let tcp_listener = TcpListener::from_std(tcp_listener)?;

    Ok(tcp_listener)
}

pub fn setup_udp_socket(bind_addr: SocketAddr, bind_device: Option<&str>) -> io::Result<UdpSocket> {
    let socket = Socket::new(
        Domain::for_address(bind_addr),
        Type::DGRAM,
        Some(Protocol::UDP),
    )?;

    setup_socket(&socket, bind_device, bind_addr)?;

    let udp_socket = std::net::UdpSocket::from(socket);

    #[cfg(all(target_os = "windows", target_env = "msvc"))]
    fix_windows_udp(&udp_socket);

    let udp_socket = UdpSocket::from_std(udp_socket)?;

    Ok(udp_socket)
}

#[allow(unused_variables)]
fn setup_socket<'a, T: Into<SockRef<'a>>>(
    socket: T,
    bind_device: Option<&str>,
    bind_addr: SocketAddr,
) -> io::Result<()> {
    let sock_ref: SockRef<'a> = socket.into();
    sock_ref.set_nonblocking(true)?;
    let sock_typ = sock_ref.r#type()?;

    if bind_addr.is_ipv6() {
        sock_ref.set_only_v6(false)?;
    }

    // https://github.com/pymumu/smartdns/blob/e26ecf6a52851f88e2937448019f74b753c0e6dc/src/dns_server/server_socket.c#L111
    if sock_typ == Type::STREAM {
        // enable TCP_FASTOPEN
        sock_ref.set_tcp_nodelay(true)?;
    }

    sock_ref.set_reuse_address(true)?;
    #[cfg(not(any(
        target_os = "solaris",
        target_os = "illumos",
        target_os = "cygwin",
        target_os = "windows"
    )))]
    sock_ref.set_reuse_port(true)?;

    #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
    if let Some(device) = bind_device {
        sock_ref.bind_device(Some(device.as_bytes()))?;
    }

    sock_ref.bind(&bind_addr.into())?;

    Ok(())
}

/// set UDP_CONNRESET off to ignore UdpSocket's WSAECONNRESET error
#[cfg(all(target_os = "windows", target_env = "msvc"))]
fn fix_windows_udp<T: std::os::windows::io::AsRawSocket>(udp_socket: &T) {
    // https://github.com/mokeyish/smartdns-rs/issues/391
    // https://github.com/shadowsocks/shadowsocks-rust/blob/3b47fa67fac6c2bded73616a284f26c6159cbe9a/src/relay/sys/windows/mod.rs#L17
    use std::ffi::c_void;
    use std::{mem, ptr};
    use windows::Win32::Foundation::FALSE;
    use windows::Win32::Networking::WinSock::{
        SIO_UDP_CONNRESET, SOCKET, SOCKET_ERROR, WSAGetLastError, WSAIoctl,
    };

    let handle = SOCKET(udp_socket.as_raw_socket() as usize);
    let mut bytes_returned: u32 = 0;
    let enable = FALSE;
    unsafe {
        let ret = WSAIoctl(
            handle,
            SIO_UDP_CONNRESET,
            Some(&enable as *const _ as *const c_void),
            mem::size_of_val(&enable) as u32,
            Some(ptr::null_mut()),
            0,
            &mut bytes_returned,
            Some(ptr::null_mut()),
            None,
        );

        if ret == SOCKET_ERROR {
            // ignore the error here, just warn and continue
            let err_code = WSAGetLastError();
            log::warn!("WSAIoctl failed with error code {:?}", err_code);
            // return Err(td::io::Error::from_raw_os_error(err_code.0));
        }
    };
}

pub trait LocalAddr {
    fn local_addr(&self) -> io::Result<SocketAddr>;
}

impl LocalAddr for TcpListener {
    #[inline]
    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.local_addr()
    }
}

impl LocalAddr for UdpSocket {
    #[inline]
    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.local_addr()
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crate::{config::IBindConfig, dns_conf::RuntimeConfig};
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    // Exercise the complete configuration-to-socket path used by Passwall.
    #[tokio::test]
    async fn device_wildcard_accepts_both_families() {
        let cfg = RuntimeConfig::builder()
            .with("bind [::]:0@lo -group China")
            .with("bind-tcp [::]:0@lo -group China")
            .build()
            .unwrap();
        let udp_config = &cfg.binds()[0];
        let tcp_config = &cfg.binds()[1];
        let udp = setup_udp_socket(udp_config.sock_addr(), udp_config.device()).unwrap();
        let tcp = setup_tcp_socket(tcp_config.sock_addr(), tcp_config.device()).unwrap();
        assert!(udp.local_addr().unwrap().ip().is_unspecified());
        assert!(tcp.local_addr().unwrap().ip().is_unspecified());

        for host in ["127.0.0.1", "::1"] {
            let ip = host.parse::<std::net::IpAddr>().unwrap();
            tokio::time::timeout(Duration::from_secs(2), async {
                let client = UdpSocket::bind((ip, 0)).await.unwrap();
                client
                    .send_to(b"dns", (ip, udp.local_addr().unwrap().port()))
                    .await
                    .unwrap();
                let mut packet = [0; 3];
                let (len, peer) = udp.recv_from(&mut packet).await.unwrap();
                assert_eq!(&packet[..len], b"dns");
                udp.send_to(&packet, peer).await.unwrap();
                let (len, _) = client.recv_from(&mut packet).await.unwrap();
                assert_eq!(&packet[..len], b"dns");

                let mut client =
                    tokio::net::TcpStream::connect((ip, tcp.local_addr().unwrap().port()))
                        .await
                        .unwrap();
                let (mut server, _) = tcp.accept().await.unwrap();
                client.write_all(b"dns").await.unwrap();
                server.read_exact(&mut packet).await.unwrap();
                assert_eq!(&packet, b"dns");
                server.write_all(b"dns").await.unwrap();
                client.read_exact(&mut packet).await.unwrap();
                assert_eq!(&packet, b"dns");
            })
            .await
            .expect("both loopback address families must work");
        }
    }
}
