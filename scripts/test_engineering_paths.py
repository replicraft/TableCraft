"""Current engineering paths fail closed without historical aliases."""
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import engineering_paths


class EngineeringPathsTests(unittest.TestCase):
    def test_repository_root_is_independent_of_tool_depth(self):
        root = engineering_paths.repository_root()
        source = root / 'tests/access/queries/expressions/tool.py'
        self.assertEqual(engineering_paths.repository_root(source), root)

    def test_explicit_recipe_is_confined_and_must_exist(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            recipe = root / 'tests/access/queries/expression/Oracle.java'
            recipe.parent.mkdir(parents=True)
            recipe.write_text('fixture')
            with patch.object(engineering_paths, 'repository_root', return_value=root):
                self.assertEqual(
                    engineering_paths.reference_file('tests/access/queries/expression/Oracle.java'),
                    recipe,
                )
                for invalid in ('../Oracle.java', str(recipe), 'scripts/Oracle.java'):
                    with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                        engineering_paths.reference_file(invalid)
                with self.assertRaises(FileNotFoundError):
                    engineering_paths.reference_file('tests/access/queries/expression/Missing.java')

    def test_bare_recipe_requires_one_current_match(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            first = root / 'tests/access/queries/expressions/Oracle.java'
            first.parent.mkdir(parents=True)
            first.write_text('first')
            with patch.object(engineering_paths, 'repository_root', return_value=root):
                self.assertEqual(engineering_paths.reference_file('Oracle.java'), first)
                with self.assertRaises(ValueError):
                    engineering_paths.reference_file('Absent.java')
                second = root / 'tests/access/rows/insert/Oracle.java'
                second.parent.mkdir(parents=True)
                second.write_text('second')
                with self.assertRaises(ValueError):
                    engineering_paths.reference_file('Oracle.java')


if __name__ == '__main__':
    unittest.main()
