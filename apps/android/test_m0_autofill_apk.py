import os
from pathlib import Path
import subprocess
import unittest
from zipfile import ZipFile


ANDROID = Path(__file__).resolve().parent
APK_DIR = ANDROID / "app/build/outputs/apk"
SPIKE_CLASSES = ("SpikeAutofillService", "SpikeLoginActivity")


class AutofillApkTest(unittest.TestCase):
    def check_variant(self, variant, filename, includes_spike):
        aapt2 = Path(os.environ["ANDROID_HOME"]) / "build-tools/35.0.0/aapt2"
        apk = APK_DIR / variant / filename
        manifest = subprocess.check_output(
            [aapt2, "dump", "xmltree", apk, "--file", "AndroidManifest.xml"], text=True
        )
        self.assertIn("com.pandex.app", manifest)
        self.assertIn(".MainActivity", manifest)
        for marker in (
            *SPIKE_CLASSES,
            "android.permission.BIND_AUTOFILL_SERVICE",
            "android.service.autofill.AutofillService",
            "android.autofill",
            '":autofill"',
        ):
            with self.subTest(marker=marker):
                self.assertEqual(marker in manifest, includes_spike)

        with ZipFile(apk) as archive:
            entries = archive.namelist()
            self.assertIn("lib/arm64-v8a/libnsh.so", entries)
            self.assertTrue(any(name.startswith("assets/nsh/") for name in entries))
            dex_files = [archive.read(name) for name in entries
                         if name.startswith("classes") and name.endswith(".dex")]
            self.assertTrue(dex_files)
            for name in SPIKE_CLASSES:
                descriptor = f"Lcom/pandex/autofill/{name};".encode()
                with self.subTest(dex_class=name):
                    self.assertEqual(any(descriptor in dex for dex in dex_files), includes_spike)

    def test_debug_keeps_autofill_and_nsh_in_one_apk(self):
        self.check_variant("debug", "app-debug.apk", True)

    def test_release_excludes_spike_components(self):
        self.check_variant("release", "app-release-unsigned.apk", False)


if __name__ == "__main__":
    unittest.main()
