# SPDX-License-Identifier: GPL-3.0-only

include $(TOPDIR)/rules.mk

PKG_NAME:=luci-app-smartdns-rs-compat
PKG_VERSION:=0.13.1
PKG_RELEASE:=1
PKG_LICENSE:=GPL-3.0-only
PKG_MAINTAINER:=SmartDNS-rs contributors
PKG_BUILD_DEPENDS:=luci-base/host

include $(INCLUDE_DIR)/package.mk

define Package/luci-app-smartdns-rs-compat
  SECTION:=luci
  CATEGORY:=LuCI
  SUBMENU:=3. Applications
  TITLE:=Lua LuCI support for SmartDNS-rs (Chinese included)
  DEPENDS:=+luci-base +smartdns-rs
  PKGARCH:=all
  CONFLICTS:=luci-app-smartdns luci-app-smartdns-rs
endef

# Build a single package, including translations, instead of luci.mk's split
# i18n packages. Old firmware already provides CBI in luci-base; modern LuCI
# users install luci-compat separately.
define Build/Prepare
	$(INSTALL_DIR) $(PKG_BUILD_DIR)
endef

define Build/Configure
endef

define Build/Compile
	po2lmo ./po/zh_Hans/smartdns.po $(PKG_BUILD_DIR)/smartdns-rs-compat.zh-cn.lmo
endef

define Package/luci-app-smartdns-rs-compat/install
	$(INSTALL_DIR) $(1)
	$(CP) ./root/* $(1)/
	$(INSTALL_DIR) $(1)/usr/lib/lua/luci/i18n
	$(INSTALL_DATA) $(PKG_BUILD_DIR)/smartdns-rs-compat.zh-cn.lmo $(1)/usr/lib/lua/luci/i18n/smartdns-rs-compat.zh-cn.lmo
endef

$(eval $(call BuildPackage,luci-app-smartdns-rs-compat))
