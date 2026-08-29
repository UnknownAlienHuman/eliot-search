# GitHub connector access

Before claiming GitHub is read-only:

1. Reload the full tool catalog with `list_resources(paths=["GitHub"])` **without a query filter**.
2. Check repository permissions with `GitHub.get_repo`; verify `permissions.push == true`.
3. If uncertain, use `GitHub.create_blob` as a harmless write probe. Do not attach it to a tree or commit.
4. Use GitHub API write actions even when the local VM has no network or Git credentials.

Never infer connector capabilities from a filtered or partially loaded tool list.
