#!/usr/bin/env bash
set -euo pipefail

if [ -z "${1:-}" ]; then
  echo "Please provide a tag."
  echo "Usage: ./release.sh v[X.Y.Z]"
  exit 1
fi

TAG="$1"
VER="${TAG#v}"

if [ -z "$(git status --porcelain)" ]; then
  echo "Working directory is clean."
else
  echo "Working directory is not clean. Please commit or stash your changes before running this script."
  exit 1
fi

echo "Bumping workspace crate versions to $VER ..."
# Bump ALL workspace members’ versions, and update inter-crate deps
cargo set-version --workspace "$VER"
cargo update

echo "Updating README.md version tags to $TAG ..."
if [ -f README.md ]; then
  perl -0777 -i -pe 's/\bv\d+\.\d+\.\d+(?:[-+][0-9A-Za-z\.-]+)?\b/'"$TAG"'/g' README.md
fi

echo "Preparing $TAG ..."

# update the changelog
git-cliff --config .cliff/default.toml --tag "$TAG" > CHANGELOG.md
git add -A && git commit -s -S -m "chore(release): prepare for $TAG"
git show

# generate a changelog for the tag message
export GIT_CLIFF_TEMPLATE="\
	{% for group, commits in commits | group_by(attribute=\"group\") %}
	{{ group | upper_first }}\
	{% for commit in commits %}
		- {% if commit.breaking %}(breaking) {% endif %}{{ commit.message | upper_first }} ({{ commit.id | truncate(length=7, end=\"\") }})\
	{% endfor %}
	{% endfor %}"

changelog=$(git-cliff --config .cliff/detailed.toml --unreleased --strip all)

git tag -s -a "$TAG" -m "Release $TAG" -m "$changelog"
git tag -v "$TAG"
echo "Done!"
echo "Now push the commit (git push) and the tag (git push --tags)."

