# Async Patterns in GPUI 0.2.2

GPUI provides async/await support via `cx.spawn()` and `cx.background_executor()`. Pattern differs from typical Tokio.

> **CRITICAL NOTE:** The project uses the **git version** of gpui (from Zed's repo), not the crates.io version. In the git version, `AsyncApp::update_entity()` returns `R` **directly** (not `Result<R>`). This means:
> - Do NOT chain `.ok()` on `update_entity` calls
> - Do NOT use `match` on `Result` from `update_entity`
> - The code examples below from the crates.io era still show `.ok()` — remove it when writing new code against the git version
>
> This also applies to `cx.update_entity()` called from within `cx.listener()` callbacks — same signature, returns `R` directly.

## Basic Background Task

```rust
cx.spawn(async move |cx| {
    // Async work here (can use .await)
    
    // Update UI (must go through cx methods)
    if let Some(state) = state_weak.upgrade() {
        cx.update_entity(&state, |s, cx| {
            s.field = new_value;
            cx.notify();
        }).ok();
    }
})
.detach();  // Drop handle, keep task alive
```

Key points:
- `cx.spawn()` takes async closure that receives `cx` parameter
- The `cx` inside closure is a different type than outer `cx` (async context)
- Must `.detach()` to drop handle and let task continue running
- Can use `.await` for async operations

## Background Executor (Blocking I/O)

For operations that block (like socket I/O), use background executor:

```rust
cx.spawn(async move |cx| {
    // Do blocking I/O on background thread
    let result = cx
        .background_executor()
        .spawn(async move {
            // This runs on a separate thread pool
            send_request(SOCKET_PATH, &req)  // Blocking!
        })
        .await;  // Wait for result
    
    // Back in async context, safe to access UI
    if let Some(state) = state_weak.upgrade() {
        cx.update_entity(&state, |s, cx| {
            // Process result
            cx.notify();
        }).ok();
    }
})
.detach();
```

Flow:
1. `cx.background_executor().spawn()` — runs future on thread pool
2. `.await` — waits for result
3. Back in main async context with result in hand
4. Update entity safely

## Polling Loop

For repeated async operations (e.g., every 1 second):

```rust
fn start_polling(state: Entity<AppState>, cx: &mut App) {
    cx.spawn(async move |cx| {
        loop {
            // Do async work
            let result = cx
                .background_executor()
                .spawn(async move {
                    send_request(SOCKET_PATH, &ControlRequest::ListPending)
                })
                .await;

            // Update state
            cx.update_entity(&state, |s, cx| {
                s.now_secs = unix_now();
                match result {
                    Ok(ControlResponse::PendingList(items)) => {
                        s.daemon_connected = true;
                        s.pending = items;
                    }
                    _ => {
                        s.daemon_connected = false;
                    }
                }
                cx.notify();
            })
            .ok();

            // Sleep before next iteration
            cx.background_executor()
                .timer(Duration::from_secs(1))
                .await;  // Blocks in background executor, not UI
        }
    })
    .detach();
}
```

Pattern:
1. Infinite `loop { ... }` in async closure
2. Fetch data via background executor
3. Update entity via `cx.update_entity()`
4. Sleep via `cx.background_executor().timer(duration).await`
5. Loop continues

**Important:** `.timer().await` blocks the background task, not the UI thread.

## Weak References for Async

Cannot hold strong refs across await points (prevents view cleanup). Use weak refs:

```rust
let state_weak = self.state.downgrade();

cx.spawn(async move |cx| {
    // Do async work
    if let Some(state) = state_weak.upgrade() {
        // Safe to use state here
        cx.update_entity(&state, |s, cx| {
            cx.notify();
        }).ok();
    }
})
.detach();
```

- `.downgrade()` → `WeakEntity<T>`
- `.upgrade()` → `Option<Entity<T>>` (not Result!)
- If entity was dropped, `upgrade()` returns `None`

## Event Handler with Async

Typical button click with async action:

```rust
Button::new("allow-btn")
    .label("Allow")
    .on_click({
        let state_weak = self.state.downgrade();
        let pending_id = item.id.clone();
        
        move |_, _, cx| {
            let pid = pending_id.clone();
            let state_weak = state_weak.clone();
            
            cx.spawn(async move |cx| {
                // Async work (blocking socket call)
                let _ = cx
                    .background_executor()
                    .spawn(async move {
                        send_request(
                            SOCKET_PATH,
                            &ControlRequest::ResolvePending {
                                pending_id: pid.clone(),
                                action: RuleAction::Allow,
                            },
                        )
                    })
                    .await;
                
                // Update state after response
                if let Some(state) = state_weak.upgrade() {
                    cx.update_entity(&state, |s, cx| {
                        s.pending.retain(|p| p.id != pid);
                        cx.notify();
                    })
                    .ok();
                }
            })
            .detach();
        }
    })
```

Flow:
1. Callback captures weak refs and data (cloned)
2. `cx.spawn(async move |cx| { ... })` — enter async context
3. Background executor for blocking I/O
4. Upgrade weak ref, update entity, notify

## Multiple Async Operations

For multiple independent async tasks, spawn multiple closures:

```rust
// Start polling task
let state_copy = state.clone();
cx.spawn(async move |cx| {
    loop {
        // Polling logic
        cx.background_executor().timer(Duration::from_secs(1)).await;
    }
}).detach();

// Start another background task
cx.spawn(async move |cx| {
    // Other work
}).detach();
```

Each `cx.spawn()` is independent. Use `cx.background_executor()` for blocking operations within each.

## Handling Errors in Async

```rust
cx.spawn(async move |cx| {
    let result = cx
        .background_executor()
        .spawn(async move {
            send_request(SOCKET_PATH, &req)
        })
        .await;

    match result {
        Ok(response) => {
            // Handle success
        }
        Err(e) => {
            // Handle error (update UI to show error state)
            if let Some(state) = state_weak.upgrade() {
                cx.update_entity(&state, |s, cx| {
                    s.error_message = format!("Failed: {e}");
                    cx.notify();
                }).ok();
            }
        }
    }
})
.detach();
```

## Key Differences from Tokio

| Aspect | GPUI | Tokio |
|--------|------|-------|
| Entry | `cx.spawn(async \| cx \| { ... })` | `tokio::spawn(async { ... })` |
| Blocking I/O | `cx.background_executor().spawn()` | Direct (on Tokio runtime) |
| Updating state | Via `cx.update_entity()` | Direct mutations |
| Weak refs | Necessary for views (auto-cleanup) | Optional pattern |
| Timer | `cx.background_executor().timer()` | `tokio::time::sleep()` |
| Error propagation | Manual `match` or `?` | `?` operator in async |

## Common Mistakes

### Forgetting `.detach()`

```rust
// WRONG: Drops task handle immediately
cx.spawn(async move |cx| {
    // May not run
}).await;  // ❌

// CORRECT: Task runs to completion
cx.spawn(async move |cx| {
    // Runs to completion
}).detach();  // ✓
```

### Holding Strong Ref Across Await

```rust
// WRONG: Cannot hold self.state across await
cx.spawn(async move |cx| {
    let state = self.state;  // ❌ Can't move
    
    // ... await ...
    
    cx.update_entity(&state, |s, cx| { /* ... */ }).ok();
})
```

Solution: Use weak ref created before spawn.

### Forgetting to Call `.notify()`

```rust
// WRONG: Entity updated but view not re-rendered
cx.update_entity(&state, |s, cx| {
    s.field = new_value;
    // ❌ Missing cx.notify()
}).ok();

// CORRECT: Triggers re-render
cx.update_entity(&state, |s, cx| {
    s.field = new_value;
    cx.notify();  // ✓
}).ok();
```

### Using `.unwrap()` on Weak Upgrade

```rust
// WRONG: Panics if entity dropped
cx.update_entity(&state_weak.upgrade().unwrap(), |s, cx| { /* ... */ }).ok();

// CORRECT: Silently handles dropped entity
if let Some(state) = state_weak.upgrade() {
    cx.update_entity(&state, |s, cx| { /* ... */ }).ok();
}
```

## Polling Pattern (Common)

For apps that need periodic updates (like decision UI polling pending decisions):

```rust
fn start_polling(state: Entity<AppState>, cx: &mut App) {
    cx.spawn(async move |cx| {
        loop {
            // Fetch data
            let result = cx
                .background_executor()
                .spawn(async move {
                    fetch_data()  // Blocking I/O
                })
                .await;

            // Update state
            cx.update_entity(&state, |s, cx| {
                // Process result
                cx.notify();
            })
            .ok();

            // Wait before next fetch
            cx.background_executor()
                .timer(Duration::from_secs(1))
                .await;
        }
    })
    .detach();
}
```

This is the pattern used in Dicto GPUI app for polling background tasks.

## Testing Async

For testing async code, use `#[tokio::test]` or `#[test]` with block_on:

```rust
#[test]
fn test_async_operation() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let result = some_async_operation().await;
        assert_eq!(result, expected);
    });
}
```

But GPUI async tests require full app context (complex). Focus on testing sync logic, mock async boundaries.
