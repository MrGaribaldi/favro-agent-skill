#!/usr/bin/env python3
"""Register a newly created human request in a shared, private poller registry."""
import argparse
import fcntl
import json
import os
from pathlib import Path
import tempfile


def register(path, action):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.with_suffix(path.suffix + '.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        state = json.loads(path.read_text()) if path.exists() else {'schema_version': 1, 'actions': {}}
        if state.get('schema_version') != 1:
            raise ValueError('Unsupported human-action registry version')
        previous = state['actions'].get(action['card_id'])
        if previous and (previous['instance_id'], previous['source']) != (action['instance_id'], action['source']):
            raise ValueError('Action already registered for a different instance/source')
        # Re-running registration must not reopen a completed or cancelled request.
        state['actions'][action['card_id']] = {**action, 'status': (previous or {}).get('status', 'outstanding')}
        descriptor, temporary = tempfile.mkstemp(dir=path.parent, prefix=path.name + '.')
        try:
            with os.fdopen(descriptor, 'w') as output:
                json.dump(state, output, ensure_ascii=False, indent=2)
                output.write('\n')
            os.replace(temporary, path)
        finally:
            if os.path.exists(temporary):
                os.unlink(temporary)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--registry', required=True, type=Path)
    for name in ('card-id', 'instance-id', 'number', 'source', 'need', 'title'):
        parser.add_argument('--' + name, required=True)
    args = vars(parser.parse_args())
    path = args.pop('registry')
    register(path, args)
    print('Registered human action #' + args['number'])


if __name__ == '__main__':
    main()
