use futures::Future;

pub trait Executable {
  fn exec(&mut self);
}

pub trait AsyncExecutable {
  type ExecResult: Future<Output = ()>;

  fn exec(&mut self) -> Self::ExecResult;
}