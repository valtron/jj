// Copyright 2020 The Jujutsu Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::io::Write as _;

use testutils::TestResult;
use testutils::git;

use crate::common::TestEnvironment;

#[test]
fn test_gitignores() -> TestResult {
    let test_env = TestEnvironment::default();
    let work_dir = test_env.work_dir("repo");
    git::init(work_dir.root());
    work_dir
        .run_jj(["git", "init", "--git-repo", "."])
        .success();

    // Say in core.excludesFiles that we don't want file1, file2, or file3
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(work_dir.root().join(".git").join("config"))?;
    // Put the file in "~/my-ignores" so we also test that "~" expands to "$HOME"
    file.write_all(b"[core]\nexcludesFile=~/my-ignores\n")?;
    drop(file);
    std::fs::write(
        test_env.home_dir().join("my-ignores"),
        "file1\nfile2\nfile3",
    )?;

    // Say in .git/info/exclude that we actually do want file2 and file3
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(work_dir.root().join(".git").join("info").join("exclude"))?;
    file.write_all(b"!file2\n!file3")?;
    drop(file);

    // Say in .gitignore (in the working copy) that we actually do not want file2
    // (again)
    work_dir.write_file(".gitignore", "file2");

    // Writes some files to the working copy
    work_dir.write_file("file0", "contents");
    work_dir.write_file("file1", "contents");
    work_dir.write_file("file2", "contents");
    work_dir.write_file("file3", "contents");

    let output = work_dir.run_jj(["diff", "-s"]);
    insta::assert_snapshot!(output, @"
    A .gitignore
    A file0
    A file3
    [EOF]
    ");
    Ok(())
}

#[test]
fn test_gitignores_relative_excludes_file_path() -> TestResult {
    let test_env = TestEnvironment::default();
    let work_dir = test_env.work_dir("repo");
    test_env
        .run_jj_in(".", ["git", "init", "--colocate", "repo"])
        .success();

    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(work_dir.root().join(".git").join("config"))?;
    file.write_all(b"[core]\nexcludesFile=../my-ignores\n")?;
    drop(file);
    std::fs::write(test_env.env_root().join("my-ignores"), "ignored\n")?;

    work_dir.write_file("ignored", "");
    work_dir.write_file("not-ignored", "");

    // core.excludesFile should be resolved relative to the workspace root, not
    // to the cwd.
    let sub_dir = work_dir.create_dir("sub");
    let output = sub_dir.run_jj(["diff", "-s"]);
    insta::assert_snapshot!(output.normalize_backslash(), @"
    A ../not-ignored
    [EOF]
    ");
    let output = test_env.run_jj_in(".", ["-Rrepo", "diff", "-s"]);
    insta::assert_snapshot!(output.normalize_backslash(), @"
    A repo/not-ignored
    [EOF]
    ");
    Ok(())
}

#[test]
fn test_gitignores_ignored_file_in_target_commit() {
    let test_env = TestEnvironment::default();
    let work_dir = test_env.work_dir("repo");
    git::init(work_dir.root());
    work_dir
        .run_jj(["git", "init", "--git-repo", "."])
        .success();

    // Create a commit with file "ignored" in it
    work_dir.write_file("ignored", "committed contents\n");
    work_dir
        .run_jj(["bookmark", "create", "-r@", "with-file"])
        .success();
    let target_commit_id = work_dir
        .run_jj(["log", "--no-graph", "-T=commit_id", "-r=@"])
        .success()
        .stdout
        .into_raw();

    // Create another commit where we ignore that path
    work_dir.run_jj(["new", "root()"]).success();
    work_dir.write_file("ignored", "contents in working copy\n");
    work_dir.write_file(".gitignore", ".gitignore\nignored\n");

    // Update to the commit with the "ignored" file
    let output = work_dir.run_jj(["edit", "with-file"]);
    insta::assert_snapshot!(output, @"
    ------- stderr -------
    Error: Failed to check out commit 3cf51c1acf24971ff49cbe03c59ee61236eafc91
    Caused by: Working copy contains ignored files that would be overwritten or deleted
    Hint: Relevant files:
      - ignored
    [EOF]
    [exit status: 1]
    ");
    let output = work_dir.run_jj(["diff", "--git", "--from", &target_commit_id]);
    insta::assert_snapshot!(output, @"
    diff --git a/ignored b/ignored
    deleted file mode 100644
    index 8a69467466..0000000000
    --- a/ignored
    +++ /dev/null
    @@ -1,1 +0,0 @@
    -committed contents
    [EOF]
    ");
}

#[test]
fn test_gitignores_file_deleted_in_target_commit() {
    let test_env = TestEnvironment::default();
    let work_dir = test_env.work_dir("repo");
    git::init(work_dir.root());
    work_dir
        .run_jj(["git", "init", "--git-repo", "."])
        .success();

    // Create a commit with file "ignored" in it
    work_dir.write_file("ignored", "committed contents\n");
    work_dir
        .run_jj(["bookmark", "create", "-r@", "with-file"])
        .success();

    // Create another commit where we ignore that path
    work_dir.run_jj(["new", "root()"]).success();
    work_dir.write_file(".gitignore", ".gitignore\nignored\n");

    // TODO: this fails because checking for mismatched files iterates over files in
    // WC, and `ignored` doesn't exist there

    // Update to the commit with the "ignored" file
    let output = work_dir.run_jj(["edit", "with-file"]);
    insta::assert_snapshot!(output, @"
    ------- stderr -------
    Error: Failed to check out commit 3cf51c1acf24971ff49cbe03c59ee61236eafc91
    Caused by: Working copy contains ignored files that would be overwritten or deleted
    Hint: Relevant files:
      - ignored
    [EOF]
    [exit status: 1]
    ");
}

#[test]
fn test_gitignores_unignored_file() {
    let test_env = TestEnvironment::default();
    let work_dir = test_env.work_dir("repo");
    git::init(work_dir.root());
    work_dir
        .run_jj(["git", "init", "--git-repo", "."])
        .success();

    work_dir.write_file("ignored-mismatch", "committed contents\n");
    work_dir.write_file("ignored-identical", "identical contents\n");
    work_dir
        .run_jj(["bookmark", "create", "-r@", "with-files"])
        .success();

    work_dir.run_jj(["new", "root()"]).success();
    work_dir.write_file(".gitignore", "ignored-*\n");
    work_dir.write_file("ignored-mismatch", "contents in working copy\n");
    work_dir.write_file("ignored-identical", "identical contents\n");

    let output = work_dir.run_jj(["edit", "with-files"]);
    insta::assert_snapshot!(output, @"
    ------- stderr -------
    Error: Failed to check out commit 0c439204b1e7c934cbe6d489a217bd4d85504c37
    Caused by: Working copy contains ignored files that would be overwritten or deleted
    Hint: Relevant files:
      ! ignored-mismatch
    [EOF]
    [exit status: 1]
    ");
}

#[test]
fn test_gitignores_checkout_target_without_ignored_file() {
    let test_env = TestEnvironment::default();
    let work_dir = test_env.work_dir("repo");
    git::init(work_dir.root());
    work_dir
        .run_jj(["git", "init", "--git-repo", "."])
        .success();

    // Create empty commit
    work_dir
        .run_jj(["bookmark", "create", "-r@", "r-commit"])
        .success();

    // Create another commit where we ignore file "unignored"
    work_dir.run_jj(["new", "root()"]).success();
    work_dir.write_file(".gitignore", "unignored\n");
    work_dir.write_file("somedir/.gitignore", "*\n");
    work_dir.write_file("somedir/ignored", "ignored\n");
    work_dir.write_file("unignored", "untracked contents\n");

    // Update to commit without "unignored"
    let output = work_dir.run_jj(["edit", "r-commit"]);
    insta::assert_snapshot!(output, @"
    ------- stderr -------
    Error: Failed to check out commit e8849ae12c709f2321908879bc724fdb2ab8a781
    Caused by: Working copy contains ignored files that would be overwritten or deleted
    Hint: Relevant files:
      + unignored
    [EOF]
    [exit status: 1]
    ");
}

#[test]
fn test_gitignores_derive_tracked_from_ignores() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    work_dir.write_file("file1.txt", "content1");
    work_dir.write_file("file2.txt", "content2");

    // No ignores, all files tracked
    let output = work_dir.run_jj(["file", "list"]);
    insta::assert_snapshot!(output, @"
    file1.txt
    file2.txt
    [EOF]
    ");
    let output = work_dir.run_jj(["status"]);
    insta::assert_snapshot!(output, @"
    Working copy changes:
    A file1.txt
    A file2.txt
    Working copy  (@) : qpvuntsm 376a8fab (no description set)
    Parent commit (@-): zzzzzzzz 00000000 (empty) (no description set)
    [EOF]
    ");

    // If a file is ignored, it gets untracked
    work_dir.write_file(".gitignore", "file1.txt\n");
    let output = work_dir.run_jj(["file", "list"]);
    insta::assert_snapshot!(output, @"
    .gitignore
    file2.txt
    [EOF]
    ");
    let output = work_dir.run_jj(["status"]);
    insta::assert_snapshot!(output, @"
    Working copy changes:
    A .gitignore
    A file2.txt
    Working copy  (@) : qpvuntsm 7b2ed7a2 (no description set)
    Parent commit (@-): zzzzzzzz 00000000 (empty) (no description set)
    [EOF]
    ");

    // If a file is unignored, it gets tracked
    work_dir.write_file(".gitignore", "\n");
    let output = work_dir.run_jj(["file", "list"]);
    insta::assert_snapshot!(output, @"
    .gitignore
    file1.txt
    file2.txt
    [EOF]
    ");
    let output = work_dir.run_jj(["status"]);
    insta::assert_snapshot!(output, @"
    Working copy changes:
    A .gitignore
    A file1.txt
    A file2.txt
    Working copy  (@) : qpvuntsm ee9547bd (no description set)
    Parent commit (@-): zzzzzzzz 00000000 (empty) (no description set)
    [EOF]
    ");
}
