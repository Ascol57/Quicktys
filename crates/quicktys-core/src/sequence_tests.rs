    use super::*;
    use crate::invoker::ActionResult;
    use serde_json::json;
    use std::sync::Mutex;

    /// Records every invocation and returns a scripted/echo result.
    struct MockInvoker {
        calls: Mutex<Vec<(String, String, Value)>>,
    }

    impl MockInvoker {
        fn new() -> Arc<Self> {
            Arc::new(MockInvoker {
                calls: Mutex::new(Vec::new()),
            })
        }
        fn calls(&self) -> Vec<(String, String, Value)> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl ActionInvoker for MockInvoker {
        fn invoke<'a>(
            &'a self,
            plugin: &'a str,
            action: &'a str,
            config: Value,
        ) -> BoxFut<'a, ActionResult> {
            Box::pin(async move {
                self.calls
                    .lock()
                    .unwrap()
                    .push((plugin.to_string(), action.to_string(), config.clone()));
                // Echo the config back so `bind` has something to capture.
                Ok(config)
            })
        }
    }

    fn runner(mock: Arc<MockInvoker>) -> Runner {
        Runner::new(Arc::new(ExprEngine::new()), mock)
    }

    fn action(plugin: &str, act: &str, config: Value) -> Node {
        Node::Action(quicktys_schema::node::ActionNode {
            plugin: plugin.into(),
            action: act.into(),
            config,
            bind: None,
        })
    }

    #[tokio::test]
    async fn progress_brackets_each_top_level_node() {
        use std::sync::Mutex as StdMutex;
        struct Recorder(StdMutex<Vec<(Vec<usize>, bool)>>);
        impl ProgressSink for Recorder {
            fn node(&self, path: &[usize], running: bool) {
                self.0.lock().unwrap().push((path.to_vec(), running));
            }
        }
        let mock = MockInvoker::new();
        let root = Node::Block {
            body: vec![
                action("a", "x", json!({})),
                action("b", "y", json!({})),
                action("c", "z", json!({})),
            ],
        };
        let rec = Arc::new(Recorder(StdMutex::new(Vec::new())));
        runner(mock.clone())
            .run_with_progress(&root, Context::new(), rec.clone())
            .await
            .unwrap();
        // Each node reports start then finish, in order, before the next starts.
        let events = rec.0.lock().unwrap().clone();
        assert_eq!(
            events,
            vec![
                (vec![0], true),
                (vec![0], false),
                (vec![1], true),
                (vec![1], false),
                (vec![2], true),
                (vec![2], false),
            ]
        );
    }

    #[tokio::test]
    async fn runs_block_sequentially() {
        let mock = MockInvoker::new();
        let root = Node::Block {
            body: vec![
                action("a", "x", json!({})),
                action("b", "y", json!({})),
            ],
        };
        runner(mock.clone())
            .run(&root, Context::new())
            .await
            .unwrap();
        let calls = mock.calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, "a");
        assert_eq!(calls[1].0, "b");
    }

    #[tokio::test]
    async fn config_templating_and_context() {
        // `{{ }}` is Handlebars interpolation; `@name` is a simple variable.
        let mock = MockInvoker::new();
        let root = action("twitch", "chat", json!({ "msg": "hi {{ user }}, you have @pts pts" }));
        let mut base = Context::new();
        base.insert("user".into(), json!("ada"));
        base.insert("pts".into(), json!(9));
        runner(mock.clone()).run(&root, base).await.unwrap();
        assert_eq!(mock.calls()[0].2, json!({ "msg": "hi ada, you have 9 pts" }));
    }

    #[tokio::test]
    async fn at_variable_and_handlebars_over_dotted_state() {
        use crate::reactivity::StateStore;
        let mock = MockInvoker::new();
        let state = Arc::new(StateStore::new());
        state.set("pear.title", json!("Nightcall"));
        state.set("pear.artists", json!(["Kavinsky", "Lovefoxxx"]));
        let runner = Runner::with_state(Arc::new(ExprEngine::new()), mock.clone(), state);

        // A lone `@path` keeps the typed value (the list stays a list).
        let a = action("p", "x", json!({ "list": "@pear.artists" }));
        runner.run(&a, Context::new()).await.unwrap();
        assert_eq!(mock.calls()[0].2["list"], json!(["Kavinsky", "Lovefoxxx"]));

        // `@` interpolation + Handlebars `{{ }}` with dotted paths and a loop.
        let b = action(
            "p",
            "y",
            json!({ "msg": "Now: @pear.title — {{#each pear.artists}}{{this}} {{/each}}" }),
        );
        runner.run(&b, Context::new()).await.unwrap();
        assert_eq!(
            mock.calls()[1].2["msg"],
            json!("Now: Nightcall — Kavinsky Lovefoxxx ")
        );
    }

    #[tokio::test]
    async fn handlebars_math_helpers() {
        let mock = MockInvoker::new();
        let root = action(
            "p",
            "x",
            json!({ "msg": "{{sub a b}} {{div a b}} {{idiv a b}} {{pow b 2}} {{sqrt c}}" }),
        );
        let mut base = Context::new();
        base.insert("a".into(), json!(10));
        base.insert("b".into(), json!(4));
        base.insert("c".into(), json!(9));
        runner(mock.clone()).run(&root, base).await.unwrap();
        // 10-4=6, 10/4=2.5, 10//4=2, 4^2=16, sqrt(9)=3.
        assert_eq!(mock.calls()[0].2["msg"], json!("6 2.5 2 16 3"));
    }

    #[tokio::test]
    async fn backslash_escapes_templates() {
        let mock = MockInvoker::new();
        let root = action(
            "p",
            "x",
            // \@ → literal @, \{{ → literal {{, \\ → literal \, plain ones resolve.
            json!({ "msg": "\\@name is @name, \\{{lit}} not {{upper name}}, path C:\\\\x" }),
        );
        let mut base = Context::new();
        base.insert("name".into(), json!("ada"));
        runner(mock.clone()).run(&root, base).await.unwrap();
        assert_eq!(
            mock.calls()[0].2["msg"],
            json!("@name is ada, {{lit}} not ADA, path C:\\x")
        );
    }

    #[tokio::test]
    async fn handlebars_text_and_comparison_helpers() {
        let mock = MockInvoker::new();
        let root = action(
            "p",
            "x",
            json!({ "msg": "{{upper name}} | {{truncate name 3}} | {{default missing name}} \
                            | {{join tags \", \"}} | {{#if (gt score 5)}}win{{else}}lose{{/if}}" }),
        );
        let mut base = Context::new();
        base.insert("name".into(), json!("kavinsky"));
        base.insert("tags".into(), json!(["synth", "wave"]));
        base.insert("score".into(), json!(9));
        runner(mock.clone()).run(&root, base).await.unwrap();
        assert_eq!(
            mock.calls()[0].2["msg"],
            json!("KAVINSKY | kav… | kavinsky | synth, wave | win")
        );
    }

    /// The same state a template reads with `@pear.title` must be readable from
    /// an `if` condition — with or without the sigil. Conditions are Rhai, where
    /// a flat `pear.title` key is not an identifier, so the context is nested
    /// before evaluation.
    #[tokio::test]
    async fn if_condition_reads_dotted_state_with_and_without_the_sigil() {
        use crate::reactivity::StateStore;
        for cond in [r#"pear.title == "Nightcall""#, r#"@pear.title == "Nightcall""#] {
            let mock = MockInvoker::new();
            let state = Arc::new(StateStore::new());
            state.set("pear.title", json!("Nightcall"));
            let runner = Runner::with_state(Arc::new(ExprEngine::new()), mock.clone(), state);

            let root = Node::If {
                cond: Expr::new(cond),
                then: Box::new(action("p", "then", json!({}))),
                otherwise: Some(Box::new(action("p", "else", json!({})))),
            };
            runner.run(&root, Context::new()).await.unwrap();
            assert_eq!(mock.calls()[0].1, "then", "condition `{cond}` should be true");
        }
    }

    #[tokio::test]
    async fn if_takes_the_true_branch() {
        let mock = MockInvoker::new();
        let root = Node::If {
            cond: Expr::new("score > 5"),
            then: Box::new(action("p", "yes", json!({}))),
            otherwise: Some(Box::new(action("p", "no", json!({})))),
        };
        let mut base = Context::new();
        base.insert("score".into(), json!(10));
        runner(mock.clone()).run(&root, base).await.unwrap();
        assert_eq!(mock.calls()[0].1, "yes");
    }

    #[tokio::test]
    async fn let_binding_feeds_later_action() {
        let mock = MockInvoker::new();
        let root = Node::Block {
            body: vec![
                Node::Let {
                    name: "greeting".into(),
                    value: Expr::new(r#""hello""#),
                },
                action("p", "say", json!("{{ greeting }}")),
            ],
        };
        runner(mock.clone())
            .run(&root, Context::new())
            .await
            .unwrap();
        assert_eq!(mock.calls()[0].2, json!("hello"));
    }

    #[tokio::test]
    async fn foreach_iterates_array() {
        let mock = MockInvoker::new();
        let root = Node::Loop(LoopNode::ForEach {
            var: "item".into(),
            list: Expr::new("[1, 2, 3]"),
            body: Box::new(action("p", "use", json!("@item"))),
            max: 100,
        });
        runner(mock.clone())
            .run(&root, Context::new())
            .await
            .unwrap();
        let configs: Vec<Value> = mock.calls().into_iter().map(|c| c.2).collect();
        assert_eq!(configs, vec![json!(1), json!(2), json!(3)]);
    }

    #[tokio::test]
    async fn repeat_over_budget_errors() {
        let mock = MockInvoker::new();
        let root = Node::Loop(LoopNode::Repeat {
            count: Expr::new("1000"),
            body: Box::new(action("p", "x", json!({}))),
            max: 10,
        });
        let err = runner(mock.clone())
            .run(&root, Context::new())
            .await
            .unwrap_err();
        assert_eq!(err, RunError::LoopBudgetExceeded { max: 10 });
        assert!(mock.calls().is_empty());
    }

    #[tokio::test]
    async fn parallel_all_runs_every_branch() {
        let mock = MockInvoker::new();
        let root = Node::Parallel {
            join: JoinPolicy::All,
            branches: vec![
                action("a", "x", json!({})),
                action("b", "y", json!({})),
                action("c", "z", json!({})),
            ],
        };
        runner(mock.clone())
            .run(&root, Context::new())
            .await
            .unwrap();
        assert_eq!(mock.calls().len(), 3);
    }

    #[tokio::test]
    async fn return_short_circuits_block() {
        let mock = MockInvoker::new();
        let root = Node::Block {
            body: vec![
                Node::Return {
                    value: Some(Expr::new("42")),
                },
                action("p", "never", json!({})),
            ],
        };
        let out = runner(mock.clone())
            .run(&root, Context::new())
            .await
            .unwrap();
        assert_eq!(out, Some(json!(42)));
        assert!(mock.calls().is_empty());
    }

    #[tokio::test]
    async fn action_error_propagates() {
        struct Failing;
        impl ActionInvoker for Failing {
            fn invoke<'a>(&'a self, _p: &'a str, _a: &'a str, _c: Value) -> BoxFut<'a, ActionResult> {
                Box::pin(async move { Err("boom".to_string()) })
            }
        }
        let runner = Runner::new(Arc::new(ExprEngine::new()), Arc::new(Failing));
        let err = runner
            .run(&action("p", "x", json!({})), Context::new())
            .await
            .unwrap_err();
        assert!(matches!(err, RunError::Action { message, .. } if message == "boom"));
    }

    #[tokio::test]
    async fn try_catches_after_retries() {
        // Body always fails; retry twice, then the catch handler runs.
        struct AlwaysFail;
        impl ActionInvoker for AlwaysFail {
            fn invoke<'a>(&'a self, p: &'a str, _a: &'a str, _c: Value) -> BoxFut<'a, ActionResult> {
                let p = p.to_string();
                Box::pin(async move {
                    if p == "boom" {
                        Err("nope".into())
                    } else {
                        Ok(Value::Null)
                    }
                })
            }
        }
        let runner = Runner::new(Arc::new(ExprEngine::new()), Arc::new(AlwaysFail));
        let root = Node::Try {
            body: Box::new(action("boom", "x", json!({}))),
            catch: Some(Box::new(action("recover", "y", json!({})))),
            retry: Some(quicktys_schema::node::Retry { max: 2, delay_ms: 0 }),
        };
        // Catch handler succeeds → run is Ok.
        let out = runner.run(&root, Context::new()).await.unwrap();
        assert_eq!(out, None);
    }

    #[tokio::test]
    async fn wait_until_resumes_when_state_satisfies_condition() {
        let mock = MockInvoker::new();
        let store = Arc::new(StateStore::new());
        store.set("phase", json!("intro"));
        let runner = Runner::with_state(Arc::new(ExprEngine::new()), mock.clone(), store.clone());

        let root = Node::Block {
            body: vec![
                Node::WaitFor(WaitNode::Until {
                    cond: Expr::new(r#"phase == "live""#),
                    timeout_ms: Some(5000),
                }),
                action("obs", "switch", json!({})),
            ],
        };

        // Drive the run and flip the state concurrently.
        let run_fut = runner.run(&root, Context::new());
        let flip = async {
            tokio::task::yield_now().await;
            store.set("phase", json!("live"));
            std::future::pending::<()>().await
        };
        let out = tokio::select! {
            r = run_fut => r,
            _ = flip => unreachable!(),
        };
        out.unwrap();
        assert_eq!(mock.calls().len(), 1);
        assert_eq!(mock.calls()[0].1, "switch");
    }

    #[tokio::test]
    async fn wait_for_event_resumes_on_matching_trigger() {
        let mock = MockInvoker::new();
        let bus = Arc::new(TriggerBus::new());
        let runner = Runner::new(Arc::new(ExprEngine::new()), mock.clone()).with_bus(bus.clone());

        let root = Node::Block {
            body: vec![
                Node::WaitFor(WaitNode::Event {
                    trigger: "chat".into(),
                    filter: Some(Expr::new(r#"user == "ada""#)),
                    timeout_ms: Some(5000),
                }),
                action("obs", "alert", json!({})),
            ],
        };

        let run_fut = runner.run(&root, Context::new());
        let emit = async {
            tokio::task::yield_now().await;
            // Non-matching first, then the match.
            bus.emit(TriggerEvent::new(
                "chat",
                json!({ "user": "bob" }).as_object().unwrap().clone(),
            ));
            bus.emit(TriggerEvent::new(
                "chat",
                json!({ "user": "ada" }).as_object().unwrap().clone(),
            ));
            std::future::pending::<()>().await
        };
        let out = tokio::select! {
            r = run_fut => r,
            _ = emit => unreachable!(),
        };
        out.unwrap();
        assert_eq!(mock.calls().len(), 1);
        assert_eq!(mock.calls()[0].1, "alert");
    }

    #[tokio::test]
    async fn wait_until_times_out_into_error() {
        let mock = MockInvoker::new();
        let store = Arc::new(StateStore::new());
        let runner = Runner::with_state(Arc::new(ExprEngine::new()), mock, store);
        let root = Node::WaitFor(WaitNode::Until {
            cond: Expr::new("ready"),
            timeout_ms: Some(30),
        });
        let err = runner.run(&root, Context::new()).await.unwrap_err();
        assert_eq!(err, RunError::WaitTimeout);
    }

    #[tokio::test]
    async fn abort_stops_a_long_delay() {
        let mock = MockInvoker::new();
        let token = Cancel::new();
        let root = Node::WaitFor(WaitNode::Delay {
            ms: Expr::new("600000"), // 10 minutes
        });
        let r = runner(mock);
        let result = tokio::select! {
            biased;
            // Cancel first, then park so this arm never resolves on its own.
            _ = async { token.cancel(); std::future::pending::<()>().await } => unreachable!(),
            out = r.run_with_cancel(&root, Context::new(), token.clone()) => out,
        };
        assert_eq!(result.unwrap_err(), RunError::Aborted);
    }
