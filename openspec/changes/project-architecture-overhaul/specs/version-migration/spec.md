## ADDED Requirements

### Requirement: Data version tracking

The application SHALL track the current data version and apply migrations when needed.

#### Scenario: Version stored
- **WHEN** database is initialized
- **THEN** current version is stored in app_config table

### Requirement: Migration system

The application SHALL have a migration system that applies incremental database schema changes.

#### Scenario: Migration applied
- **WHEN** application detects a new version
- **THEN** migration scripts are executed in order

### Requirement: Migrations are idempotent

Migration scripts SHALL be idempotent (can be safely executed multiple times).

#### Scenario: Migration rerun safe
- **WHEN** a migration is executed twice
- **THEN** no errors occur and data remains consistent

### Requirement: Migration rollback

The migration system SHALL support rollback for failed migrations.

#### Scenario: Rollback on failure
- **WHEN** a migration fails
- **THEN** the database is rolled back to the previous state

### Requirement: Migration logging

The application SHALL log migration progress and results.

#### Scenario: Migration logged
- **WHEN** a migration is executed
- **THEN** progress is logged at INFO level
