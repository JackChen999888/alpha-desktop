"""Exercise the generated Windows hook command using isolated profiles and data."""
import json, os, pathlib, subprocess, sys, uuid

bins = pathlib.Path(sys.argv[1]).resolve()
root = pathlib.Path(sys.argv[2]).resolve() / ('hooks-' + uuid.uuid4().hex)
env = dict(os.environ, DESKTOP_ISLAND_DATA_DIR=str(root / 'data'),
           CODEX_HOME=str(root / 'codex'), CLAUDE_CONFIG_DIR=str(root / 'claude'))
subprocess.run([str(bins / 'desktop-island-event.exe'), '--install-hooks'],
               env=env, capture_output=True, check=True, timeout=20)
for folder, file, agent in [('codex', 'hooks.json', 'Codex'), ('claude', 'settings.json', 'Claude Code')]:
    config = json.loads((root / folder / file).read_text(encoding='utf-8'))
    command = config['hooks']['UserPromptSubmit'][0]['hooks'][0]['command']
    payload = json.dumps({'session_id': agent + '-test', 'cwd': r'G:\测试项目\Alpha',
                          'hook_event_name': 'UserPromptSubmit', 'prompt': 'NEVER_PERSIST'}, ensure_ascii=False)
    result = subprocess.run(['powershell.exe', '-NoProfile', '-NonInteractive', '-Command', command],
                            input=payload, encoding='utf-8', capture_output=True, env=env, timeout=15)
    assert result.returncode == 0, result.stderr
    assert result.stdout.strip() == '{}', repr(result.stdout)
events = [json.loads(p.read_text(encoding='utf-8')) for p in (root / 'data' / 'pro' / 'events').glob('*.json')]
assert {e['agent'] for e in events} == {'Codex', 'Claude Code'}, events
assert all(e['project'] == r'G:\测试项目\Alpha' and e['phase'] == 'running' for e in events), events
assert 'NEVER_PERSIST' not in json.dumps(events)
print('PASS: generated Windows commands, both agents, UTF-8 paths, observer output, no prompt capture')

# Exercise the deployed event executable with isolated, synthetic task payloads.
def emit(agent, session, event, project, **extra):
    payload = dict(session_id=session, hook_event_name=event, cwd=project,
                   prompt='NEVER_PERSIST', **extra)
    subprocess.run([str(bins / 'desktop-island-event.exe'), '--agent-event', agent],
                   input=json.dumps(payload, ensure_ascii=False), encoding='utf-8',
                   capture_output=True, check=True, env=env, timeout=15)

for agent in ['Codex', 'Claude Code']:
    emit(agent, 'multi-a', 'UserPromptSubmit', 'G:/fixture-a')
    emit(agent, 'multi-b', 'UserPromptSubmit', 'G:/fixture-b')
    emit(agent, 'multi-a', 'PermissionRequest', 'G:/fixture-a')
    emit(agent, 'multi-a', 'PreToolUse', 'G:/fixture-a')
    emit(agent, 'multi-a', 'SubagentStart', 'G:/fixture-a', agent_id='child')
    emit(agent, 'multi-a', 'SubagentStop', 'G:/fixture-a', agent_id='child')
    emit(agent, 'multi-b', 'Stop', 'G:/fixture-b')
    if agent == 'Codex':
        emit(agent, 'failure', 'PostModelRequest', 'G:/fixture-c', status='failed')
    else:
        emit(agent, 'failure', 'StopFailure', 'G:/fixture-c')
    emit(agent, 'waiting', 'PermissionRequest', 'G:/fixture-d')
events = [json.loads(p.read_text(encoding='utf-8')) for p in (root / 'data' / 'pro' / 'events').glob('*.json')]
latest = {}
for e in events:
    key = (e['agent'], e['session'])
    if key not in latest or e['at'] > latest[key]['at']:
        latest[key] = e
for agent in ['Codex', 'Claude Code']:
    for session, phase in [('multi-a','running'),('multi-a / child','completed'),
                           ('multi-b','completed'),('failure','failed'),('waiting','attention')]:
        assert latest[(agent,session)]['phase'] == phase, latest
    assert latest[(agent,'multi-a')]['project'] != latest[(agent,'multi-b')]['project']
assert 'NEVER_PERSIST' not in json.dumps(events)
print('PASS: two agents, independent projects/children, running/completed/attention/failed, metadata only')
