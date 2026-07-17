-- Example: active contacts by team
SELECT team, COUNT(*) AS contact_count
FROM contacts
GROUP BY team
ORDER BY contact_count DESC;
