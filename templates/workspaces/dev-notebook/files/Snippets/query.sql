-- Open issues ordered by priority
SELECT title, status, priority, component
FROM issues
WHERE status IN ('open', 'in_progress')
ORDER BY
  CASE priority
    WHEN 'high' THEN 1
    WHEN 'medium' THEN 2
    ELSE 3
  END,
  title;
