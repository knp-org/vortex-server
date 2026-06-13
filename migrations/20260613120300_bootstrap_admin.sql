-- Lock-out safeguard for installs that predate admin roles: if users already exist
-- but none is an admin, promote the earliest-created user to admin so the system can
-- still create new accounts. On a fresh install (no users) this is a no-op and the
-- first admin is created via the /auth/setup bootstrap instead.
UPDATE users SET role = 'admin'
WHERE id = (SELECT id FROM users ORDER BY id LIMIT 1)
  AND NOT EXISTS (SELECT 1 FROM users WHERE role = 'admin');
