#!/bin/bash
[ -z "$1" ] && echo "Usage: $0 <monitor-url>" && exit 2
STATUS=$(curl -s -o /dev/null -w "%{http_code}" "$1/health")
[ "$STATUS" -eq 200 ] && exit 0 || exit 1
