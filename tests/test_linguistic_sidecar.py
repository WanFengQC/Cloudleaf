"""类型侧包的许可/人工字段均为明确自编fixture，不是真实标注或频率。"""
import copy
import math
import unittest

from tools.data_pipeline.linguistic_sidecar import validate_sidecar

SHA = 'a' * 64


def package():
    return {'format':'cloudleaf-linguistic-sidecar-v1', 'schema_version':1, 'synthetic':True,
            'lexicon_sha256':SHA, 'sources':[{'id':'fixture', 'version':'1', 'sha256':SHA,
            'license_id':'MIT', 'license_sha256':SHA, 'audit_sha256':SHA,
            'review_status':'approved', 'scope':['frequency','linguistic-annotation']}],
            'entries':[{'entry_id':'entry:a','pos':None}], 'observations':[], 'senses':[],
            'translations':[], 'phrases':[]}


def observation(metric='token_frequency'):
    return {'id':'observation:1','entry_id':'entry:a','source_id':'fixture','metric':metric,
            'coverage':'observed','value':0.1,'numerator':10,'denominator':100,
            'denominator_unit':'tokens', 'transform':None,
            'corpus':{'id':'fixture-corpus','version':'1','sha256':SHA,
                      'period':['2025-01-01','2025-12-31'],
                      'tokenizer':{'id':'fixture-tokenizer','version':'1','config_sha256':SHA}}}


def provenance():
    return {'source_id':'fixture','evidence':{'unit_id':'fixture-unit','sha256':SHA},
            'review':{'reviewers':['reviewer-a','reviewer-b'],'disputed':False,'adjudicator':None}}


def sense(identity='sense:1', entry='entry:a'):
    return {'id':identity,'entry_id':entry,'language':'zh','definition':'自编义项',
            'pos':None, **provenance()}


class SidecarTests(unittest.TestCase):
    def check(self, item):
        return validate_sidecar(item, {'entry:a','entry:b'}, expected_lexicon_sha256=SHA)

    def test_empty_unknown_annotations_and_observed_zero_remain_distinct(self):
        item = package()
        self.assertEqual(self.check(item)['annotated_senses'], 0)
        zero = observation()
        zero.update(value=0.0,numerator=0)
        unknown = observation()
        unknown.update(id='observation:2',coverage='unobserved',value=None,numerator=None,denominator=None)
        item['observations'] = [zero,unknown]
        result = self.check(item)
        self.assertEqual(result['unknown_observations'], 1)
        self.assertEqual(result['observed_zero'], 1)
        self.assertFalse(result['language_accuracy_valid'])

    def test_token_and_document_units_denominators_and_finite_values(self):
        item = package()
        df = observation('document_frequency')
        df.update(value=10,denominator_unit='documents')
        item['observations'] = [df]
        self.check(item)
        for field,value in [('metric','token_frequency'),('denominator_unit','tokens'),
                            ('denominator',0),('numerator',True),('value',float('nan')),
                            ('value',float('inf')),('coverage','unobserved')]:
            invalid = copy.deepcopy(item)
            invalid['observations'][0][field] = value
            with self.subTest(field=field,value=value), self.assertRaises(ValueError):
                self.check(invalid)

    def test_zipf_uses_positive_token_parent_and_explicit_base(self):
        item = package()
        base = observation()
        z = observation('zipf')
        z.update(id='observation:z',value=8.0,numerator=None,denominator=None,
                 denominator_unit='per_billion_tokens',
                 transform={'kind':'derived-token','parent_id':'observation:1',
                            'log_base':10,'scale':1000000000})
        item['observations'] = [base,z]
        self.check(item)
        for change in ['df','zero','missing-parent','wrong-value','wrong-base']:
            invalid = copy.deepcopy(item)
            if change == 'df': invalid['observations'][0].update(metric='document_frequency',value=10,denominator_unit='documents')
            elif change == 'zero': invalid['observations'][0].update(value=0.0,numerator=0)
            elif change == 'missing-parent': invalid['observations'][1]['transform']['parent_id']='observation:missing'
            elif change == 'wrong-value': invalid['observations'][1]['value']=0.0
            else: invalid['observations'][1]['transform']['log_base']=2
            with self.subTest(change=change), self.assertRaises(ValueError): self.check(invalid)

    def test_native_zipf_requires_definition_and_missing_value_cannot_be_default_zero(self):
        item = package()
        z=observation('zipf')
        z.update(value=0.0,numerator=None,denominator=None,denominator_unit='per_billion_tokens',
                 transform={'kind':'native','definition_sha256':SHA,'log_base':10,'scale':1000000000})
        item['observations']=[z]
        self.check(item)
        z['coverage']='below_cutoff'
        with self.assertRaises(ValueError):self.check(item)
        z['value']=None
        self.check(item)

    def test_zipf_cannot_relabel_another_entry_corpus_source_or_preprocessing(self):
        item=package()
        base=observation()
        derived=observation('zipf')
        derived.update(id='observation:z',value=8.0,numerator=None,denominator=None,
                       denominator_unit='per_billion_tokens',transform={'kind':'derived-token',
                       'parent_id':'observation:1','log_base':10,'scale':1000000000})
        item['observations']=[base,derived]
        self.check(item)
        for change in ['entry','source','corpus-id','corpus-sha','period','tokenizer']:
            invalid=copy.deepcopy(item)
            changed=invalid['observations'][1]
            if change=='entry':changed['entry_id']='entry:b'
            elif change=='source':
                source=copy.deepcopy(invalid['sources'][0]);source['id']='fixture-other'
                invalid['sources'].append(source);changed['source_id']='fixture-other'
            elif change=='corpus-id':changed['corpus']['id']='another-corpus'
            elif change=='corpus-sha':changed['corpus']['sha256']='b'*64
            elif change=='period':changed['corpus']['period']=['2026-01-01','2026-10-01']
            else:changed['corpus']['tokenizer']['config_sha256']='b'*64
            with self.subTest(change=change),self.assertRaises(ValueError):self.check(invalid)

    def test_sense_translation_phrase_links_and_review_are_explicit(self):
        item=package()
        item['senses']=[sense(),sense('sense:2','entry:b')]
        item['translations']=[{'id':'translation:1','source_sense_id':'sense:1',
                               'target_sense_id':'sense:2','relation':'related',**provenance()}]
        item['phrases']=[{'id':'phrase:1','entry_id':'entry:a','kind':'collocation',
                         'components':['entry:a','entry:b'],**provenance()}]
        self.assertEqual(self.check(item)['translation_links'],1)
        for change in ['orphan','same-reviewer','unapproved','unknown-entry','unsupported-relation','missing-adjudicator']:
            invalid=copy.deepcopy(item)
            if change=='orphan':invalid['translations'][0]['target_sense_id']='sense:missing'
            elif change=='same-reviewer':invalid['senses'][0]['review']['reviewers']=['a','a']
            elif change=='unapproved':invalid['sources'][0]['review_status']='pending'
            elif change=='unknown-entry':invalid['phrases'][0]['components']=['entry:missing']
            elif change=='unsupported-relation':invalid['translations'][0]['relation']='automatically-equivalent'
            else:invalid['senses'][0]['review']['disputed']=True
            with self.subTest(change=change),self.assertRaises(ValueError):self.check(invalid)

    def test_identity_scope_and_count_type_cannot_be_relabelled(self):
        item=package()
        item['observations']=[observation()]
        for change in ['duplicate','lexicon','scope','period','tokenizer','unknown-zero']:
            invalid=copy.deepcopy(item)
            if change=='duplicate':invalid['observations'].append(observation())
            elif change=='lexicon':invalid['lexicon_sha256']='b'*64
            elif change=='scope':invalid['sources'][0]['scope']=['linguistic-annotation']
            elif change=='period':invalid['observations'][0]['corpus']['period']=['2025-12-31','2025-01-01']
            elif change=='tokenizer':invalid['observations'][0]['corpus']['tokenizer']['config_sha256']='unknown'
            else:invalid['observations'][0].update(coverage='unobserved',numerator=0,value=0.0)
            with self.subTest(change=change),self.assertRaises(ValueError):self.check(invalid)

    def test_positive_count_float_underflow_is_not_an_observed_zero(self):
        item=package()
        row=observation()
        row.update(numerator=1,denominator=10**400,value=0.0)
        item['observations']=[row]
        with self.assertRaises(ValueError):self.check(item)
        row.update(numerator=0)
        self.assertEqual(self.check(item)['observed_zero'],1)


if __name__=='__main__':unittest.main()
