## ADDED Requirements

### Requirement: Data storage using SQLite

The application SHALL use SQLite as the primary data storage mechanism, replacing JSON file storage.

#### Scenario: Database file created
- **WHEN** application starts for the first time
- **THEN** `kettd.db` file is created with all necessary tables

### Requirement: SQLite schema includes all entities

The database schema SHALL include tables for tasks, subtasks, notes, reminders, history, and app configuration.

#### Scenario: All tables exist
- **WHEN** database is initialized
- **THEN** tables for tasks, subtasks, notes, reminders, history, and app_config exist

### Requirement: Data migration from JSON to SQLite

The application SHALL automatically migrate data from the existing JSON file to SQLite on first run.

#### Scenario: Migration successful
- **WHEN** application starts with existing JSON data
- **THEN** data is migrated to SQLite and JSON file is backed up

### Requirement: Database operations support transactions

All write operations (create, update, delete) SHALL be performed within transactions to ensure data consistency.

#### Scenario: Transaction rollback on error
- **WHEN** an error occurs during a write operation
- **THEN** the transaction is rolled back and data remains consistent

### Requirement: Complex queries supported

The database SHALL support complex queries (filtering by date, type, status, etc.).

#### Scenario: Query tasks by category
- **WHEN** querying tasks with category "工作"
- **THEN** only tasks with that category are returned
