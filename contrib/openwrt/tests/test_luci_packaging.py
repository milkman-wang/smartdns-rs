#!/usr/bin/env python3
"""Exercise Windows template line endings through the actual Lua IPK builder."""

import io
from pathlib import Path
import runpy
import shutil
import tarfile
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[3]
PACKAGE = Path("contrib/openwrt/luci-app-smartdns-rs-compat")
TEMPLATE = Path("root/usr/lib/lua/luci/view/smartdns/status.htm")
build = runpy.run_path(str(ROOT / "contrib/openwrt/tools/build_prebuilt_ipk.py"))["build_luci_compat"]


class LuaPackagingTest(unittest.TestCase):
    def test_windows_template_is_shipped_with_lf(self):
        with tempfile.TemporaryDirectory() as temporary:
            repo = Path(temporary)
            shutil.copytree(ROOT / PACKAGE, repo / PACKAGE)
            template = repo / PACKAGE / TEMPLATE
            expected = template.read_bytes().replace(b"\r\n", b"\n")
            template.write_bytes(expected.replace(b"\n", b"\r\n"))

            packages = build(repo, repo / "packages")
            self.assertEqual(len(packages), 1, "Compat must not need a separate translation package")
            application = packages[0]
            with tarfile.open(application, "r:gz") as package:
                with tarfile.open(fileobj=io.BytesIO(package.extractfile("./control.tar.gz").read()), mode="r:gz") as control_archive:
                    control = control_archive.extractfile("./control").read().decode()
                    self.assertIn("Conflicts: luci-app-smartdns, luci-app-smartdns-rs\n", control)
                    self.assertNotIn("luci-i18n-smartdns-rs-compat-zh-cn", control)
                with tarfile.open(fileobj=io.BytesIO(package.extractfile("./data.tar.gz").read()), mode="r:gz") as data:
                    installed = data.extractfile("./" + TEMPLATE.relative_to("root").as_posix()).read()
                    self.assertNotIn("./usr/lib/lua/luci/i18n/smartdns.zh-cn.lmo", data.getnames(),
                                     "Do not overwrite the catalog owned by older language packages")
                    translation = data.extractfile("./usr/lib/lua/luci/i18n/smartdns-rs-compat.zh-cn.lmo").read()
                    self.assertIn("常规设置".encode(), translation)
                    self.assertIsNotNone(data.getmember("./www/luci-static/resources/smartdns/form.css"))
            self.assertEqual(installed, expected)
            self.assertNotIn(b"\r", installed)


if __name__ == "__main__":
    unittest.main()
