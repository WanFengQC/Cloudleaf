"""资源合同只验证实际观测，不实现搜索算法副本。"""
import copy
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from search_resource_contract import validate_case, validate_run


class ResourceContractTests(unittest.TestCase):
    def fixture(self):
        case = {"id": "a", "family": "fixture", "input": "a", "kind": "fixture"}
        config = {"method": "exact", "output_k": 5, "max_edges": 10, "max_expansions": 10, "max_resident_paths": 10}
        row = {**case, "status": "ok", "raw_fallback": "a", "fallback_preserved": True, "guaranteed": True, "candidates": [],
               "statistics": {"budget": None, "beam_pruned": 0, "graph_edges": 1, "expansions": 1, "resident_high_water": 1}}
        return case, config, row

    def test_budget_pruning_and_fallback_contracts(self):
        case, config, row = self.fixture()
        validate_case(row, case, config)
        for key, value in (("budget", "Expansions"), ("beam_pruned", 1), ("resident_high_water", 11)):
            changed = copy.deepcopy(row)
            changed["statistics"][key] = value
            with self.assertRaises(ValueError):
                validate_case(changed, case, config)
        row["raw_fallback"] = ""
        with self.assertRaises(ValueError):
            validate_case(row, case, config)

    def test_partial_reference_is_not_complete_but_dag_discards_partial(self):
        case, config, row = self.fixture()
        row.update(guaranteed=False, candidates=[{"text": "甲"}])
        row["statistics"]["budget"] = "Expansions"
        with self.assertRaises(ValueError):
            validate_case(row, case, config)
        config["method"] = "reference"
        validate_case(row, case, config)

    def test_version_configuration_denominator_and_input_binding(self):
        case, config, row = self.fixture()
        run = {"format": "cloudleaf-search-resource-observation-v2", "configuration": config, "cases": [row]}
        validate_run(run, [case], config)
        for changed in ({**run, "format": "old"}, {**run, "configuration": {}}, {**run, "cases": []},
                        {**run, "cases": [{**row, "input": "other"}]}):
            with self.assertRaises(ValueError):
                validate_run(changed, [case], config)


if __name__ == "__main__":
    unittest.main()
