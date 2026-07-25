---
title: Feedback program
export_policy: allow
---


# Feedback program

Customer feedback enters through `CRM/Feedback.data → Forms → Feedback intake`.
The operational record remains editable in SQLite. A form-submitted workflow
creates a governed proposal for a narrative triage note.

This separates:

- the customer’s raw signal;
- product classification and ownership;
- derived narrative;
- approval of any workspace mutation.

Use `CRM.data` for contact and company relations. Use `CRM/Feedback.data` for
the feedback lifecycle.
