"""Actual storage/task methods with explicit in-memory Redis and Celery adapters."""
from pathlib import Path
import importlib.util
import sys
import types
import unittest

ROOT = Path(__file__).resolve().parent

def load(name, path):
    spec = importlib.util.spec_from_file_location(name, ROOT / path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module

package = types.ModuleType('celery_dedupe'); package.__path__ = [str(ROOT / 'celery_dedupe')]
sys.modules['celery_dedupe'] = package
base = load('celery_dedupe.storage', 'celery_dedupe/storage/base.py')
base.__path__ = [str(ROOT / 'celery_dedupe/storage')]
RedisStorage = load('celery_dedupe.storage.redis', 'celery_dedupe/storage/redis.py').RedisStorage

class CeleryTaskAdapter:
    fail_publish = False
    publish_calls = 0
    def apply_async(self, **kwargs):
        self.publish_calls += 1
        if self.fail_publish:
            raise OSError('synthetic broker failure')
        return ('published', kwargs['task_id'])
    def _get_app(self):
        return types.SimpleNamespace(conf=types.SimpleNamespace(CELERY_ALWAYS_EAGER=False))
    def AsyncResult(self, task_id):
        return ('async-result', task_id)

celery = types.ModuleType('celery'); celery.Task = CeleryTaskAdapter
sys.modules['celery'] = celery
result = types.ModuleType('celery.result'); result.EagerResult = object
sys.modules['celery.result'] = result
utils = types.ModuleType('celery.utils'); utils.uuid = lambda: 'synthetic-id'
sys.modules['celery.utils'] = utils
DedupeTask = load('celery_dedupe.tasks', 'celery_dedupe/tasks.py').DedupeTask

class ConnectionAdapter:
    def __init__(self):
        self.values = {}; self.expiries = {}; self.fail_expire = False
        self.expire_result = True; self.expire_on_get = False
    def setnx(self, key, value):
        if key in self.values: return False
        self.values[key] = value; return True
    def expire(self, key, seconds):
        if self.fail_expire: raise OSError('synthetic expiry failure')
        if self.expire_result: self.expiries[key] = seconds
        return self.expire_result
    def get(self, key):
        if self.expire_on_get: self.values.pop(key, None)
        return self.values.get(key)
    def delete(self, key):
        self.values.pop(key, None)

class ControlledTask(DedupeTask):
    # Lock lifecycle only; the original Python2 key builder is not emulated.
    def _create_key(self, args, kwargs): return 'controlled-key'

class Review(unittest.TestCase):
    def test_expire_error_leaves_non_expiring_lock(self):
        c = ConnectionAdapter(); c.fail_expire = True
        with self.assertRaises(OSError): RedisStorage(c, expiry=10).obtain_lock('k','owner')
        self.assertEqual(c.values, {'k':'owner'}); self.assertEqual(c.expiries,{})
    def test_expire_false_is_ignored(self):
        c = ConnectionAdapter(); c.expire_result = False
        self.assertTrue(RedisStorage(c, expiry=10).obtain_lock('k','owner'))
        self.assertEqual(c.expiries,{})
    def test_old_owner_release_deletes_new_owner(self):
        c = ConnectionAdapter(); c.values['k']='new-owner'
        RedisStorage(c).release_lock('k'); self.assertNotIn('k',c.values)
    def test_storage_configuration_is_not_shared(self):
        c = ConnectionAdapter(); a = RedisStorage(c,expiry=10); b = RedisStorage(c)
        self.assertEqual(a.config['expiry'],10); self.assertEqual(b.config['expiry'],0)
    def task(self,c):
        t = ControlledTask(); t.storage = RedisStorage(c); return t
    def test_publish_error_leaves_lock(self):
        c = ConnectionAdapter(); t = self.task(c); t.fail_publish = True
        with self.assertRaises(OSError): t.apply_async(task_id='old-owner')
        self.assertEqual(c.values['controlled-key'],'old-owner')
    def test_expired_lock_returns_result_with_none_id(self):
        c = ConnectionAdapter(); c.values['controlled-key']='old-owner'; c.expire_on_get=True
        self.assertEqual(self.task(c).apply_async(task_id='new-owner'),('async-result',None))
    def test_old_completion_removes_replacement_lock(self):
        c = ConnectionAdapter(); c.values['controlled-key']='new-owner'
        self.task(c).on_success(None,'old-owner',(),{})
        self.assertNotIn('controlled-key',c.values)
    def test_same_id_retry_republishes(self):
        c = ConnectionAdapter(); c.values['controlled-key']='owner'; t = self.task(c)
        self.assertEqual(t.apply_async(task_id='owner'),('published','owner'))
        self.assertEqual(t.publish_calls,1)
    def test_python3_does_not_supply_python2_iteritems(self):
        with self.assertRaises(AttributeError): DedupeTask()._create_key(None,None)

if __name__ == '__main__': unittest.main(verbosity=2)
