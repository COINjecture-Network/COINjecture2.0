-- Enable UUID generation
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
-- Enable crypto functions
CREATE EXTENSION IF NOT EXISTS "pgcrypto";
-- Enable pg_trgm for fuzzy search on marketplace
CREATE EXTENSION IF NOT EXISTS "pg_trgm";
