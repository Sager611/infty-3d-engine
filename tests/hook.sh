#!/usr/bin/env bash
set -euo pipefail

variables=$(git rev-parse --local-env-vars)
while IFS= read -r variable; do
  unset "${variable}"
done <<< "${variables}"

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
before=$(git -C "${root}" status --porcelain)
repo=$(mktemp -d)
marker=$(mktemp)
trap 'rm -rf "${repo}" "${marker}"' EXIT

if [[ ${1:-} != isolated ]]; then
  GIT_INDEX_FILE="${marker}" bash "${BASH_SOURCE[0]}" isolated > /dev/null
  [[ ! -s ${marker} ]]
fi

git -C "${repo}" init -q
git -C "${repo}" config user.email test@example.com
git -C "${repo}" config user.name test
mkdir "${repo}/.githooks"
cp "${root}/.githooks/pre-commit" "${repo}/.githooks/pre-commit"
chmod +x "${repo}/.githooks/pre-commit"
cat > "${repo}/check" <<'EOF'
#!/usr/bin/env bash
printf invoked >> "${CHECK_MARKER:?}"
EOF
chmod +x "${repo}/check"
git -C "${repo}" add .githooks/pre-commit check
run_hook() {
  (cd -- "${repo}" && CHECK_MARKER="${marker}" .githooks/pre-commit)
}

printf staged > "${repo}/staged.rs"
git -C "${repo}" add staged.rs
run_hook
marker_contents=$(<"${marker}")
[[ "${marker_contents}" == invoked ]]

printf bad > "${repo}/bad.rs"
git -C "${repo}" add bad.rs
printf corrected > "${repo}/bad.rs"
set +e
output=$(run_hook 2>&1)
hook_rc=$?
set -e
if (( hook_rc == 0 )); then
  printf '%s\n' 'unstaged correction was accepted' >&2
  exit 1
fi
[[ "${output}" == *'Stage all non-ignored changes before committing.'* ]]

printf new > "${repo}/new-module.rs"
set +e
output=$(run_hook 2>&1)
hook_rc=$?
set -e
if (( hook_rc == 0 )); then
  printf '%s\n' 'untracked file was accepted' >&2
  exit 1
fi
[[ "${output}" == *'Stage all non-ignored changes before committing.'* ]]
marker_contents=$(<"${marker}")
[[ "${marker_contents}" == invoked ]]

after=$(git -C "${root}" status --porcelain)
[[ "${before}" == "${after}" ]]
printf '%s\n' 'hook tests passed'
