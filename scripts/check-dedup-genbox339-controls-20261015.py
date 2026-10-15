#!/usr/bin/env python3
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
DIRECTORY = ROOT / 'docs/research/dedup-500'
EVIDENCE = DIRECTORY / 'evidence/Genbox--FindDupes'
WORK = Path('/private/tmp/rrrah-genbox339-controls-20261015')
WORK.mkdir(exist_ok=False)
OUTPUT = DIRECTORY / 'genbox339-controls-20261015.json'
assert not OUTPUT.exists()

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

registry = json.loads((DIRECTORY / 'reviews/339.json').read_text())
rows = registry['available_source_evidence'] + registry['available_readme_license_evidence']
rows += json.loads(Path('/private/tmp/rrrah-genbox339-local-20261015.json').read_text())
pins = {}
for row in rows:
    source = EVIDENCE / row['local']
    assert sha(source) == row['sha256']
    if row['path'].startswith('src/FindDupes/') and row['path'].endswith('.cs'):
        target = WORK / row['path']
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
        pins[str(source)] = sha(source)
harness = DIRECTORY / 'genbox339-controls-20261015.cs'
shutil.copyfile(harness, WORK / 'Probe.cs')
(WORK / 'global.json').write_text(json.dumps({'sdk': {'version': '8.0.425', 'rollForward': 'disable'}}))
(WORK / 'Controls.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net8.0</TargetFramework><StartupObject>Probe</StartupObject><AllowUnsafeBlocks>true</AllowUnsafeBlocks><UseSharedCompilation>false</UseSharedCompilation></PropertyGroup></Project>')
(WORK / 'NuGet.Config').write_text('<configuration><packageSources><clear /></packageSources></configuration>')
environment = os.environ.copy()
environment.update(DOTNET_CLI_HOME=str(WORK / 'cli-home'), DOTNET_CLI_TELEMETRY_OPTOUT='1', DOTNET_SKIP_FIRST_TIME_EXPERIENCE='1', DOTNET_GENERATE_ASPNET_CERTIFICATE='false', NUGET_PACKAGES=str(WORK / 'packages'), MSBuildEnableWorkloadResolver='false')
report = dict(status='running', producer_sha256=sha(__file__), source_pins=pins, harness_sha256=sha(harness), commands=[], scope='All original production C# files copied byte unchanged; real installed .NET8 SHA1/xxHash/LINQ/FileStream/reflection, explicit compile-only CommandLine adapter. Original CLI entrypoint, parser, deletion and UI are never called. Six upstream hash vectors reproduced manually; original xUnit runner not executed. Synthetic absent paths and metadata records only, no real digest collision claim, fullnetcore2.2/package/platform/resource/quality or production adoption qualification.')
def save():
    OUTPUT.write_text(json.dumps(report, indent=2) + '\n')
save()
commands = [('restore', ['dotnet', 'restore', 'Controls.csproj', '--configfile', str(WORK / 'NuGet.Config')]), ('build', ['dotnet', 'build', 'Controls.csproj', '--no-restore', '-c', 'Release', '-m:1', '-p:UseSharedCompilation=false']), ('controls', ['dotnet', str(WORK / 'bin/Release/net8.0/Controls.dll')])]
for name, command in commands:
    result = subprocess.run(command, cwd=WORK, env=environment, capture_output=True, text=True)
    log = WORK / (name + '.log')
    log.write_text(result.stdout + result.stderr)
    report['commands'].append(dict(name=name, command=command, returncode=result.returncode, log=str(log), log_sha256=sha(log)))
    if result.returncode:
        report['status'] = 'failed_' + name
        save()
        print(log.read_text())
        raise SystemExit(result.returncode)
    save()
assert all(sha(path) == digest for path, digest in pins.items())
report.update(status='passed_original_production_controls', stdout=result.stdout, executable_sha256=sha(WORK / 'bin/Release/net8.0/Controls.dll'))
save()
print(result.stdout)
