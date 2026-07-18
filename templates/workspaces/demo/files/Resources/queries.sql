-- Example read-only queries for CRM contacts (illustrative; run outside Lattice).
SELECT name, email, company, status, due_date
FROM contacts
WHERE status = 'Active'
ORDER BY due_date ASC
LIMIT 25;

SELECT status, COUNT(*) AS count
FROM contacts
GROUP BY status
ORDER BY count DESC;

SELECT company, COUNT(*) AS contacts
FROM contacts
WHERE company IS NOT NULL
GROUP BY company
HAVING contacts > 1;
