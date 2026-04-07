/// Mock [`CommandRunner`] for testing service lifecycle functions.
///
/// Each call records the `(bin, args)` tuple and returns the next
/// pre-programmed result from its internal queue.
#[cfg(test)]
pub(crate) mod testing {
    use std::cell::RefCell;

    use crate::error::Error;
    use crate::util::process::CommandRunner;

    /// A recorded invocation of the mock runner.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Call {
        pub bin: String,
        pub args: Vec<String>,
    }

    /// Pre-programmed response for a mock call.
    #[derive(Debug, Clone)]
    pub enum MockResponse {
        /// `run_check` returns this bool.
        Check(bool),
        /// `run_output` returns this stdout string.
        Output(String),
        /// Return this error from whichever method is called.
        Err(Error),
        /// `run_exec` returns Ok(()).
        ExecOk,
    }

    /// A controllable mock for [`CommandRunner`].
    pub struct MockCommandRunner {
        responses: RefCell<Vec<MockResponse>>,
        calls: RefCell<Vec<Call>>,
    }

    impl MockCommandRunner {
        /// Create a new mock with the given pre-programmed responses.
        ///
        /// Responses are consumed in FIFO order.
        pub fn new(responses: Vec<MockResponse>) -> Self {
            Self {
                responses: RefCell::new(responses),
                calls: RefCell::new(Vec::new()),
            }
        }

        /// Return all recorded calls.
        pub fn calls(&self) -> Vec<Call> {
            self.calls.borrow().clone()
        }

        fn record(&self, bin: &str, args: &[&str]) {
            self.calls.borrow_mut().push(Call {
                bin: bin.to_owned(),
                args: args.iter().map(|s| (*s).to_owned()).collect(),
            });
        }

        fn next_response(&self) -> MockResponse {
            let mut responses = self.responses.borrow_mut();
            assert!(
                !responses.is_empty(),
                "MockCommandRunner: no more responses queued (calls so far: {:?})",
                self.calls.borrow()
            );
            responses.remove(0)
        }
    }

    impl CommandRunner for MockCommandRunner {
        fn run_check(&self, bin: &str, args: &[&str]) -> Result<bool, Error> {
            self.record(bin, args);
            match self.next_response() {
                MockResponse::Check(ok) => Ok(ok),
                MockResponse::Err(e) => Err(e),
                other => panic!("MockCommandRunner: run_check got unexpected response: {other:?}"),
            }
        }

        fn run_output(&self, bin: &str, args: &[&str]) -> Result<String, Error> {
            self.record(bin, args);
            match self.next_response() {
                MockResponse::Output(s) => Ok(s),
                MockResponse::Err(e) => Err(e),
                other => panic!("MockCommandRunner: run_output got unexpected response: {other:?}"),
            }
        }

        fn run_exec(&self, bin: &str, args: &[&str]) -> Result<(), Error> {
            self.record(bin, args);
            match self.next_response() {
                MockResponse::ExecOk => Ok(()),
                MockResponse::Err(e) => Err(e),
                other => panic!("MockCommandRunner: run_exec got unexpected response: {other:?}"),
            }
        }
    }
}
