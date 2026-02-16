# Release Asset Recovery

Use this runbook when a GitHub release exists but has no binary assets.

## Symptom

- Release page (for example `v0.1.1`) is present.
- Assets list is empty.
- No workers start after recreating the release in the GitHub UI.

## Why this happens in this repo

`/Users/cruzluna/Documents/Projects/jkl2/.github/workflows/release.yml` is triggered by:

```yaml
on:
  push:
    tags:
      - "*"
```

That means:

- Creating/editing/deleting a GitHub Release object does not trigger the workflow.
- The workflow runs when a tag push event happens.

## Recovery path A (preferred): rerun the existing tag workflow

If the tag already had a release workflow run, rerun it so assets are uploaded again.

```bash
# 1) Find the release workflow run for the tag
gh run list -R cruzluna/jkl-2 --workflow release.yml --limit 50 \
  --json databaseId,headBranch,status,conclusion,createdAt,url \
  --jq '.[] | select(.headBranch=="v0.1.1")'

# 2) Rerun that workflow
gh run rerun <run_id> -R cruzluna/jkl-2

# 3) Wait for completion
gh run watch <run_id> -R cruzluna/jkl-2 --exit-status

# 4) Verify assets
gh release view v0.1.1 -R cruzluna/jkl-2 --json assets,url
```

## Recovery path B: push the tag again

Use this when no prior run exists for the tag, or you intentionally want a fresh trigger.

```bash
# Point local tag to desired commit
git tag -f v0.1.1 <commit_sha>

# Force-push that tag so GitHub receives a new tag push event
git push origin refs/tags/v0.1.1 --force
```

If the remote tag does not exist, a normal push is enough:

```bash
git push origin refs/tags/v0.1.1
```

## Preventing repeats

- For the same tag, rerun the workflow instead of deleting/recreating the release.
- For code changes after publishing, prefer a new patch tag (`v0.1.2`) over reusing `v0.1.1`.

## References

- [GitHub Actions: events that trigger workflows](https://docs.github.com/en/actions/writing-workflows/choosing-when-your-workflow-runs/events-that-trigger-workflows)
- [GitHub Actions: re-running workflows and jobs](https://docs.github.com/en/actions/managing-workflow-runs/re-running-workflows-and-jobs)
