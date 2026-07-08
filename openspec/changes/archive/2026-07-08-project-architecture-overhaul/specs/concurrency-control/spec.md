## ADDED Requirements

### Requirement: Shared state must be protected by RwLock

The shared application state SHALL be protected by RwLock to prevent race conditions when multiple windows access data concurrently.

#### Scenario: Multiple reads allowed simultaneously
- **WHEN** two windows request tasks at the same time
- **THEN** both requests complete successfully without data corruption

#### Scenario: Write blocks reads
- **WHEN** one window is saving data
- **THEN** other windows wait for the write to complete before reading

### Requirement: Lock acquisition must have error handling

All lock acquisition operations SHALL include proper error handling for poisoning scenarios.

#### Scenario: Lock poisoning handled
- **WHEN** a lock is poisoned due to panic
- **THEN** the application gracefully handles the error and continues operation

### Requirement: Lock scope must be minimized

Lock acquisition SHALL be held for the minimum necessary duration to avoid blocking other operations.

#### Scenario: Lock released after operation
- **WHEN** a database operation completes
- **THEN** the lock is immediately released
