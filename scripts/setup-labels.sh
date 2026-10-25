#!/usr/bin/env bash
set -euo pipefail

REPO="jaoppb/tools"

echo "Configuring labels for $REPO..."

# 1. Delete default GitHub labels and deprecated labels if they exist
DEPRECATED_LABELS=(
  "bug"
  "documentation"
  "duplicate"
  "enhancement"
  "good first issue"
  "help wanted"
  "invalid"
  "question"
  "wontfix"
  "epic"
  "accessibility"
)

for label in "${DEPRECATED_LABELS[@]}"; do
  if gh label list --repo "$REPO" | grep -Fwq "$label"; then
    echo "Deleting deprecated label: $label"
    gh label delete "$label" --repo "$REPO" --yes || true
  fi
done

# 2. Define desired labels: name | color | description
LABELS=(
  # Types (Exactly one per issue)
  "type:feat|1d76db|New behaviour or capability"
  "type:fix|d73a4a|Something is broken"
  "type:chore|8b949e|Tooling, CI, housekeeping, dependencies"
  "type:docs|0075ca|Documentation, guides, specifications"
  "type:refactor|5319e7|Restructure without changing behaviour"
  "type:spike|fbca04|Time-boxed research that answers a question"

  # Areas (At least one per issue)
  "area:core|2ea44f|crates/core: domain models, github client, config"
  "area:project-sync|0e8a16|crates/project-sync: status synchronization tool"
  "area:issues-check|5319e7|crates/issues-check: issue rules and integrity auditor"
  "area:tooling|2ea44f|Flake, Nix, Just, CI, Cargo workspace"
  "area:docs|0075ca|Documentation, workflow guides, rules"
)

# 3. Create or update labels
for item in "${LABELS[@]}"; do
  IFS="|" read -r name color desc <<< "$item"
  if gh label list --repo "$REPO" | grep -Fwq "$name"; then
    echo "Updating label: $name"
    gh label edit "$name" --repo "$REPO" --color "$color" --description "$desc"
  else
    echo "Creating label: $name"
    gh label create "$name" --repo "$REPO" --color "$color" --description "$desc"
  fi
done

echo "Labels setup complete!"
