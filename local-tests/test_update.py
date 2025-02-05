#!/bin/env python
import os
import subprocess
import sys
from os.path import abspath, join
from time import sleep
import jsonrpcclient

from chainrunner import Chain, Seq, generate_keys, check_finalized

# Path to working directory, where chainspec, logs and nodes' dbs are written:
workdir = abspath(os.getenv('WORKDIR', '/tmp/workdir'))
# Path to the pre-update aleph-node binary:
oldbin = abspath(os.getenv('OLD_BINARY', join(workdir, 'aleph-node-old')))
# Path to the post-update aleph-node binary:
newbin = abspath(os.getenv('NEW_BINARY', join(workdir, 'aleph-node-new')))
# Path to the post-update compiled runtime:
runtime = abspath(os.getenv('NEW_RUNTIME', join(workdir, 'aleph_runtime.compact.wasm')))
# Path to cliain:
CLIAIN = abspath('../bin/cliain/target/release/cliain')


def query_runtime_version(nodes):
    print('Current version:')
    versions = set()
    for i, node in enumerate(nodes):
        sysver = node.rpc('system_version').result
        resp = node.rpc('state_getRuntimeVersion')
        if isinstance(resp, jsonrpcclient.Ok):
            rt = resp.result['specVersion']
            versions.add(rt)
        else:
            rt = "ERROR"
        print(f'  Node {i}: system: {sysver}  runtime: {rt}')
    if len(versions) > 1:
        print(f'ERROR: nodes reported different runtime versions: {versions}')
    if versions:
        return max(versions)
    return -1


phrases = ['//Cartman', '//Stan', '//Kyle', '//Kenny']
keys = generate_keys(newbin, phrases)
chain = Chain(workdir)
print('Bootstraping the chain with old binary')
chain.bootstrap(oldbin,
                keys.values(),
                sudo_account_id=keys[phrases[0]],
                chain_type='local')

chain.set_flags('validator',
                port=Seq(30334),
                ws_port=Seq(9944),
                rpc_port=Seq(9933),
                unit_creation_delay=200,
                execution='Native')

print('Starting the chain with old binary')
chain.start('old')

print('Waiting 90s')
sleep(90)

check_finalized(chain)
query_runtime_version(chain)

print('Killing node 3 and deleting its database')
chain[3].stop()  # OH MY GOD THEY KILLED KENNY!
chain[3].purge()

print('Restarting node 3 with new binary')
chain[3].binary = newbin
chain[3].start('new3')

print('Waiting 30s')
sleep(30)

check_finalized(chain)
oldver = query_runtime_version(chain)

print('Submitting extrinsic with new runtime')
subprocess.check_call(
    [CLIAIN, '--node', 'localhost:9945', '--seed', phrases[0],
        'update-runtime', '--runtime', runtime],
    env=dict(os.environ, RUST_LOG="warn"))

print('Waiting a bit')
sleep(10)

check_finalized(chain)
newver = query_runtime_version(chain)

print('Restarting remaining nodes with new binary')
chain.stop(nodes=[0, 1, 2])
chain.set_binary(newbin, nodes=[0, 1, 2])
print('Waiting 30s')
sleep(30)
chain.start('new', nodes=[0, 1, 2])

print('Waiting 90s')
sleep(90)

check_finalized(chain)
query_runtime_version(chain)

print('Stopping the chain')
chain.stop()

print('Waiting a bit')
sleep(10)

hf = min(node.highest_block()[1] for node in chain)
print(f'Sanity check: the highest finalized block is {hf}. '
      f'Comparing exported states after that block:')
if chain[0].state(hf) == chain[1].state(hf) == chain[2].state(hf) == chain[3].state(hf):
    print("The same :)")
else:
    print("DIFFERENT!")
    sys.exit(1)

if oldver == newver:
    print("ERROR: runtime version reported by nodes didn't change after the update")
    sys.exit(1)
