# Monorepo Breakup Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Break the simse monorepo into 13 private repos, create a cloud aggregator repo, and restructure the main repo with submodules.

**Architecture:** Use `git subtree split` to extract per-folder history into branches, push each to a new private GitHub repo, then replace folders with submodules. Cloud services get an additional aggregator repo.

**Tech Stack:** git, gh CLI, git submodules

---

### Task 1: Clean up stale folders

**Step 1: Delete stale folders with no source code**

```bash
cd /home/dev/simse
rm -rf simse-cloud simse-mcp simse-analytics
```

**Step 2: Commit**

```bash
git add -A
git commit -m "chore: remove stale simse-cloud, simse-mcp, simse-analytics folders"
```

---

### Task 2: Split and push all 12 source folders to individual repos

For each of these 12 folders, perform the subtree split, create a GitHub repo, and push:

```
simse-api, simse-app, simse-auth, simse-bi, simse-cdn,
simse-landing, simse-mailer, simse-payments, simse-status,
simse-core, simse-cli, simse-brand
```

**Step 1: Run the split-and-push loop**

For each folder `$NAME`:

```bash
cd /home/dev/simse

# Extract history for this folder into a branch
git subtree split --prefix=$NAME -b split-$NAME

# Create private repo on GitHub
gh repo create shmor3/$NAME --private --confirm

# Push the split branch as main to the new repo
git push https://github.com/shmor3/$NAME.git split-$NAME:main

# Clean up the local branch
git branch -D split-$NAME
```

Run this for all 12 folders. The exact loop:

```bash
cd /home/dev/simse
for NAME in simse-api simse-app simse-auth simse-bi simse-cdn \
            simse-landing simse-mailer simse-payments simse-status \
            simse-core simse-cli simse-brand; do
    echo "=== Processing $NAME ==="
    git subtree split --prefix=$NAME -b split-$NAME
    gh repo create shmor3/$NAME --private --confirm
    git push https://github.com/shmor3/$NAME.git split-$NAME:main
    git branch -D split-$NAME
    echo "=== Done: $NAME ==="
done
```

**Step 2: Verify all 12 repos exist**

```bash
for NAME in simse-api simse-app simse-auth simse-bi simse-cdn \
            simse-landing simse-mailer simse-payments simse-status \
            simse-core simse-cli simse-brand; do
    gh repo view shmor3/$NAME --json name -q '.name' 2>/dev/null && echo " OK" || echo " MISSING: $NAME"
done
```

Expected: All 12 print OK.

---

### Task 3: Create the simse-cloud aggregator repo

**Step 1: Create the repo**

```bash
gh repo create shmor3/simse-cloud --private --confirm
```

**Step 2: Clone it, add 9 cloud service submodules**

```bash
cd /tmp
git clone https://github.com/shmor3/simse-cloud.git
cd simse-cloud

# Initialize with a README
echo "# simse-cloud\n\nCloud services aggregator." > README.md
git add README.md
git commit -m "initial commit"

# Add all 9 cloud service repos as submodules
for NAME in simse-api simse-app simse-auth simse-bi simse-cdn \
            simse-landing simse-mailer simse-payments simse-status; do
    git submodule add https://github.com/shmor3/$NAME.git $NAME
done

git commit -m "feat: add cloud service submodules"
git push origin main
```

**Step 3: Verify submodules**

```bash
git submodule status
```

Expected: 9 submodules listed.

**Step 4: Clean up**

```bash
cd /home/dev/simse
rm -rf /tmp/simse-cloud
```

---

### Task 4: Remove source folders from main repo

**Step 1: Remove all 12 folders that are now separate repos**

```bash
cd /home/dev/simse
git rm -rf simse-api simse-app simse-auth simse-bi simse-cdn \
           simse-landing simse-mailer simse-payments simse-status \
           simse-core simse-cli simse-brand
```

**Step 2: Commit**

```bash
git commit -m "refactor: remove folders now in separate repos"
```

---

### Task 5: Add submodules to main repo

**Step 1: Add the 4 submodules**

```bash
cd /home/dev/simse

git submodule add https://github.com/shmor3/simse-core.git simse-core
git submodule add https://github.com/shmor3/simse-cli.git simse-cli
git submodule add https://github.com/shmor3/simse-brand.git simse-brand
git submodule add https://github.com/shmor3/simse-cloud.git simse-cloud
```

**Step 2: Verify submodules**

```bash
git submodule status
```

Expected: 4 submodules listed.

**Step 3: Verify workspace still compiles**

```bash
cargo check --workspace 2>&1 | tail -5
```

Expected: simse-core and simse-cli compile (the submodule checkouts should have the same code).

**Step 4: Commit**

```bash
git commit -m "feat: add simse-core, simse-cli, simse-brand, simse-cloud as submodules"
```

---

### Task 6: Clean up root files and push

**Step 1: Update .gitmodules if needed**

Verify `/home/dev/simse/.gitmodules` looks correct (4 entries).

**Step 2: Push main repo**

```bash
git push origin preview
```

**Step 3: Final verification**

```bash
# Check repo structure
ls /home/dev/simse/
# Should show: CLAUDE.md, Cargo.toml, LICENSE, README.md, deployment/, docs/,
#              simse-core/, simse-cli/, simse-brand/, simse-cloud/

# Check submodules are populated
git submodule foreach 'echo $name: $(ls | head -3)'

# Check workspace builds
cargo check --workspace 2>&1 | tail -5
```
