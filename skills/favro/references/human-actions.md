# Human action cards

Use this workflow when the user designates a board/list for their tasks. Resolve the destination, existing lane, participant, priority field and archive convention from project instructions. Do not create extra boards or lanes. The agent sets native priority according to the impact on further work, so the human can choose what to complete from a useful ordered queue.

## Create or update a request

- Keep the source work card and its owning role. Create a separate action card on the human's board for each independently completable request. Before creating it, read the source's linked requests and the destination board; update an existing open request for the same source, need and action rather than duplicating reminders across agents.
- Use an imperative title, prefixed with the need for visibility: `[PR] Review and merge … (#123)`, `[Question] Choose … (#123)`, `[Test] Check … (#123)`. Apply a native tag: `need:pr`, `need:question`, `need:test`, `need:approval`, `need:access`, or a specific `need:<kind>` such as `need:payment`; use `need:other` only when no clear kind fits. Tags are distinct from native priority.
- Assign the configured human and set native priority deliberately. Use High for unanswered questions, decisions or access that block further authorized work. Routine PR review/merge is normally Medium; a nonblocking manual test or optional follow-up is normally Low. Raise a PR/test when it actually gates the next step, release or another workstream; lower informational/nonblocking requests. Explain the impact in one sentence on the card. Reserve the highest/critical tier for a concrete urgent problem, not normal waiting. Respect explicit priority overrides from the user; do not manufacture deadlines.
- Make the body self-contained: the exact action or numbered questions, why it is needed, source card number and a native card link or verified Favro URL, relevant PR/test URLs, necessary alternatives/evidence, and what counts as finished. A test request includes the entry URL, role/account, steps and expected result. A PR request links the PR, or a branch comparison when PR creation is unavailable. Attach decision documents that the human cannot otherwise read.
- Put a reciprocal link on the source. Where the work actually waits for this action, add/update its `👤 Needs you` checklist and use Waiting (expected input) or Blocked (unexpected impediment). Preserve other requests and existing ticks. Do not move an entire workstream to Waiting when other authorized work can continue.
- Immediately register the action's common ID, per-board instance ID, source ID, need and title in the project's shared human-action registry, if its poller uses one. Do this before treating the handoff as delivered, so an archive before the next poll is still detectable. Use the shared main workspace, not an isolated worktree's local state. Do not publish credentials or signed download links in this registry.

Example (substitute existing board/lane and actual IDs):

```bash
favro add --board "Human actions" --lane "Inbox" --title "[Test] Verify signup (#123)" --body "Source: #123 <working Favro URL>\nAction: …\nTest URL: …\nExpected result: …"
favro tag --card <action-id> --add need:test
favro assign --card <action-id> --add owner
favro comment --card <source-id> --text "Human test request: #456 <working Favro URL>."
# Only when the source actually depends on completing this request:
favro depend --card <source-id> --on <action-id>
```

For a registry-based poller, register with the supplied helper (safe to repeat):

```bash
python3 <skill-path>/scripts/register-human-action.py --registry <shared-project>/.favro/local/human-actions.json --card-id <action-common-id> --instance-id <action-cardId> --number <action-number> --source <source-common-id> --need test --title "[Test] Verify signup (#123)"
```

The registry is private local state, shared across agents; keep it out of Git. Registration preserves completed/cancelled status. When intentionally cancelling or retiring a duplicate, set its registry status to `cancelled` before archiving it.

## Archive means the human action is complete

Apply this only where the human explicitly chose this convention. The human archives the request when done; they need not move it through Review/Done or add another completion comment. The agent must not archive an open request merely to infer human approval. An agent may retire a duplicate/cancelled request only with an explicit reason; exclude such retirement from human-completion events.

During normal checks, inspect tracked outstanding action cards even when their comment count has not changed. Retain their instance IDs and query archived instances: disappearing from a live listing, deletion, a lookup failure or a move to another board is not proof of completion. Confirm `archived: true` on the tracked instance. Archive events must be durably queued before advancing the registry checkpoint; retry failed checks later and deduplicate by action ID and completion transition. Do not increase the project's polling frequency.

When an archived request is confirmed, read its final answers/test observations and the related work card. Record that the human completed this action, resolve only the corresponding source checklist item, and continue the already-authorized next step. Do not recreate the same request or unarchive it as a reminder. Preserve human text and ticks on other requests. If the source owner is another role, leave the completion evidence for that role rather than claiming their implementation is done.

Completion of the human action does not imply every source acceptance condition passed. Check a PR's actual merge status and test evidence as appropriate. If a decision's content is still missing, record the completed action and ask only the remaining concrete question as a new linked request; never guess an answer from archival alone.
