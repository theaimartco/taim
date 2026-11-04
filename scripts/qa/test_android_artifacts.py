"""Prevent shared Cargo caches from substituting another Android app."""
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import tomllib
import unittest

ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location("artifact_paths",ROOT/'scripts/android/templates/artifact_paths.py')
artifacts=importlib.util.module_from_spec(spec);spec.loader.exec_module(artifacts)


class AndroidArtifactTests(unittest.TestCase):
    def test_a_stale_generic_module_cannot_satisfy_the_application_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);output=root/'aarch64-linux-android/release';output.mkdir(parents=True)
            (output/'libtaim_app.so').write_bytes(b'wrong application')
            metadata={'target_directory':str(root),'packages':[{'manifest_path':str(root/'Cargo.toml'),'targets':[{'crate_types':['cdylib'],'name':'taim_app_actual'}]}]}
            with self.assertRaisesRegex(ValueError,'own cdylib is missing'):
                artifacts.cdylib_output(metadata,root/'Cargo.toml','aarch64-linux-android','release')
            wanted=output/'libtaim_app_actual.so';wanted.write_bytes(b'right application')
            self.assertEqual(artifacts.cdylib_output(metadata,root/'Cargo.toml','aarch64-linux-android','release'),wanted)
            with self.assertRaises(ValueError):
                artifacts.cdylib_output(metadata,root/'different/Cargo.toml','aarch64-linux-android','release')

    def test_distinct_generated_projects_cannot_collide_even_with_the_same_android_id(self):
        with tempfile.TemporaryDirectory() as directory:
            names=[]
            for index,package in enumerate(['com.example.first','com.example.second','com.example.first']):
                project=Path(directory)/str(index)
                subprocess.run(['python3',str(ROOT/'scripts/android/project.py'),str(project),'--package',package],check=True,capture_output=True)
                names.append(tomllib.loads((project/'rust/Cargo.toml').read_text())['package']['name'])
                self.assertTrue((project/'tools/artifact_paths.py').is_file())
            self.assertEqual(len(set(names)),3)


if __name__=='__main__':unittest.main()
