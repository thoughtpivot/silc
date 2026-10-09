#!/usr/bin/env bash
# Cut a semantic release for the tested commit, then back-merge main into dev.
set -euo pipefail

expected_name="Dan Stephenson"
expected_email="ispyhumanfly@gmail.com"

if [ "${GIT_AUTHOR_NAME:-}" != "$expected_name" ] \
  || [ "${GIT_AUTHOR_EMAIL:-}" != "$expected_email" ] \
  || [ "${GIT_COMMITTER_NAME:-}" != "$expected_name" ] \
  || [ "${GIT_COMMITTER_EMAIL:-}" != "$expected_email" ]; then
  echo "Release commits must be ${expected_name} <${expected_email}>." >&2
  echo "Set GIT_AUTHOR_NAME, GIT_AUTHOR_EMAIL, GIT_COMMITTER_NAME, and GIT_COMMITTER_EMAIL." >&2
  exit 1
fi

if [ -z "${GITHUB_TOKEN:-}" ]; then
  echo "No release token. Configure the GitHub App or SILC_RELEASE_TOKEN. See docs/RELEASING.md." >&2
  exit 1
fi

if [ -z "${GITHUB_REPOSITORY:-}" ] || [ -z "${RELEASE_BRANCH:-}" ] || [ -z "${RELEASE_SHA:-}" ]; then
  echo "RELEASE_BRANCH, RELEASE_SHA, and GITHUB_REPOSITORY are required." >&2
  exit 1
fi

cd "$(dirname "$0")/.."

git remote set-url origin "https://x-access-token:${GITHUB_TOKEN}@github.com/${GITHUB_REPOSITORY}.git"
git fetch --force --tags origin "${RELEASE_BRANCH}"
if ! git fetch origin dev; then
  echo "origin/dev does not exist."
fi

tip="$(git rev-parse "origin/${RELEASE_BRANCH}")"
if [ "$tip" != "${RELEASE_SHA}" ]; then
  echo "${RELEASE_BRANCH} moved past ${RELEASE_SHA}; skipping this release run."
  exit 0
fi

git checkout -B "${RELEASE_BRANCH}" "${RELEASE_SHA}"

stable_tag_exists() {
  git tag --list | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+$'
}

if [ "${RELEASE_BRANCH}" = "main" ]; then
  if ! stable_tag_exists; then
    version="$(python3 scripts/bump-workspace-version.py --print)"
    echo "No stable tag yet. Anchoring v${version} at ${RELEASE_SHA}."
    git tag -a "v${version}" -m "chore(release): ${version}"
    git push origin "refs/tags/v${version}"
  fi
elif ! stable_tag_exists; then
  echo "No stable tag yet. Skipping the dev prerelease until main has vX.Y.Z."
  exit 0
fi

rm -f /tmp/silc-released-version
npx --no-install semantic-release

if [ "${RELEASE_BRANCH}" != "main" ] || [ ! -f /tmp/silc-released-version ]; then
  exit 0
fi

if ! git rev-parse --verify --quiet refs/remotes/origin/dev >/dev/null; then
  echo "No dev branch; skipping back-merge."
  exit 0
fi

version="$(tr -d '[:space:]' < /tmp/silc-released-version)"
git fetch origin main dev
git checkout -B dev origin/dev
git merge --no-ff origin/main -m "chore: back-merge main into dev after v${version}"
git push origin HEAD:dev
