---
title: Proposals
---

# Proposals

Reviewable pages land here after you **approve** a proposal from the Proposals
inbox (native desktop).

The First Look automation path:

1. Submit **CRM.data → Forms → Contact intake**.
2. `Automations/Contact intake.workflow.yaml` runs `Tasks/ContactIntakeHello.task`,
   then creates a page-create proposal for `Proposals/Contact intake follow-up.md`.
3. Approve in the inbox — this folder gains the follow-up page.
