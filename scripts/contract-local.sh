#!/usr/bin/env bash
# Local contract run for the Docker adapter against a throwaway Colima profile.
# Never touches the `default` profile; the test profile is deleted on exit.
set -euo pipefail

PROFILE="telima-test-d"
SOCKET="$HOME/.colima/$PROFILE/docker.sock"

cleanup() {
  colima delete -f "$PROFILE" || true
}
trap cleanup EXIT

colima start "$PROFILE" --cpus 2 --memory 2 --disk 10
TELIMA_DOCKER_SOCKET="$SOCKET" cargo test -p telima-docker --test contract -- --ignored
