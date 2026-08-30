-- Physis Pro PostgreSQL initialization
-- Creates the schema for API key management and usage tracking

-- Enable extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- API keys table (shared across multi-tenant instances)
CREATE TABLE IF NOT EXISTS api_keys (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    key_hash VARCHAR(128) NOT NULL UNIQUE,
    tier VARCHAR(20) NOT NULL DEFAULT 'starter',
    jobs_monthly INTEGER NOT NULL DEFAULT 100,
    jobs_used INTEGER NOT NULL DEFAULT 0,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT NOW(),
    last_used_at TIMESTAMP
);

-- API usage tracking table
CREATE TABLE IF NOT EXISTS api_usage (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    api_key_id UUID REFERENCES api_keys(id) ON DELETE CASCADE,
    endpoint VARCHAR(100) NOT NULL,
    job_cost NUMERIC(10, 3) NOT NULL DEFAULT 0.001,
    status VARCHAR(20) NOT NULL DEFAULT 'success',
    response_time_ms INTEGER,
    created_at TIMESTAMP DEFAULT NOW(),
    request_id VARCHAR(128)
);

-- Tenant state tracking (per X-Physis-User)
CREATE TABLE IF NOT EXISTS tenant_state (
    user_id VARCHAR(128) PRIMARY KEY,
    runtime_state_json JSONB DEFAULT '{}'::jsonb,
    last_accessed TIMESTAMP DEFAULT NOW(),
    created_at TIMESTAMP DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_api_keys_key_hash ON api_keys(key_hash);
CREATE INDEX IF NOT EXISTS idx_api_keys_tier ON api_keys(tier);
CREATE INDEX IF NOT EXISTS idx_api_keys_active ON api_keys(is_active);
CREATE INDEX IF NOT EXISTS idx_api_usage_key ON api_usage(api_key_id);
CREATE INDEX IF NOT EXISTS idx_api_usage_endpoint ON api_usage(endpoint);
CREATE INDEX IF NOT EXISTS idx_api_usage_created ON api_usage(created_at);
