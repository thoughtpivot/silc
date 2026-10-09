const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

function hasRemoteBranch(name) {
  try {
    execFileSync("git", ["rev-parse", "--verify", "--quiet", `refs/remotes/origin/${name}`], {
      stdio: "ignore",
    });
    return true;
  } catch {
    return false;
  }
}

// dev is optional. semantic-release fetches every configured branch, so it is
// listed only when origin/dev exists. Prerelease tags are vX.Y.Z-dev.N.
const branches = ["main"];
if (hasRemoteBranch("dev")) {
  branches.push({ name: "dev", prerelease: "dev" });
}

const changelog = fs
  .readFileSync(path.join(__dirname, "CHANGELOG.md"), "utf8")
  .replace(/^\uFEFF/, "")
  .trimStart();
const headingAt = changelog.search(/^## /m);
if (headingAt < 0) {
  throw new Error("CHANGELOG.md is missing a version heading");
}
const changelogTitle = changelog.slice(0, headingAt).trim();

module.exports = {
  branches,
  tagFormat: "v${version}",
  plugins: [
    [
      "@semantic-release/commit-analyzer",
      {
        preset: "conventionalcommits",
        releaseRules: [
          { breaking: true, release: "major" },
          { type: "feat", release: "minor" },
          { type: "fix", release: "patch" },
          { type: "chore", release: false },
        ],
      },
    ],
    [
      "@semantic-release/release-notes-generator",
      {
        preset: "conventionalcommits",
      },
    ],
    [
      "@semantic-release/changelog",
      {
        changelogFile: "CHANGELOG.md",
        changelogTitle,
      },
    ],
    [
      "@semantic-release/exec",
      {
        prepareCmd: "python3 scripts/bump-workspace-version.py ${nextRelease.version}",
        successCmd: "printf '%s' '${nextRelease.version}' > /tmp/silc-released-version",
      },
    ],
    [
      "@semantic-release/git",
      {
        assets: ["CHANGELOG.md", "Cargo.toml", "Cargo.lock"],
        message: "chore(release): ${nextRelease.version}",
      },
    ],
    [
      "@semantic-release/github",
      {
        successComment: false,
        failComment: false,
        failTitle: false,
        releasedLabels: false,
      },
    ],
  ],
};
