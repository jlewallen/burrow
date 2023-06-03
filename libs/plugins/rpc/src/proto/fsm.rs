use tracing::*;

use super::Sender;

pub enum Transition<S, M> {
    None,
    Direct(S),
    Send(M, S),
    #[allow(dead_code)]
    SendOnly(M),
}

impl<S, M> Transition<S, M> {
    pub fn map_message<O, F>(self, mut f: F) -> Transition<S, O>
    where
        F: FnMut(M) -> O,
    {
        match self {
            Transition::None => Transition::<S, O>::None,
            Transition::Direct(s) => Transition::<S, O>::Direct(s),
            Transition::Send(m, s) => Transition::<S, O>::Send(f(m), s),
            Transition::SendOnly(m) => Transition::<S, O>::SendOnly(f(m)),
        }
    }
}

#[derive(Debug)]
pub struct Machine<S> {
    pub state: S,
}

#[allow(dead_code)]
impl<S> Machine<S>
where
    S: std::fmt::Debug,
{
    pub fn apply<M>(
        &mut self,
        transition: Transition<S, M>,
        sender: &mut Sender<M>,
    ) -> anyhow::Result<()>
    where
        M: std::fmt::Debug,
    {
        match transition {
            Transition::None => {
                debug!("(none) {:?}", &self.state);
                Ok(())
            }
            Transition::Direct(state) => {
                debug!("(direct) {:?} -> {:?}", &self.state, &state);
                self.state = state;
                Ok(())
            }
            Transition::Send(sending, state) => {
                trace!("(send) {:?}", &sending);
                sender.send(sending)?;
                debug!("(send) {:?} -> {:?}", &self.state, &state);
                self.state = state;
                Ok(())
            }
            Transition::SendOnly(sending) => {
                trace!("(send-only) {:?}", &sending);
                sender.send(sending)?;
                Ok(())
            }
        }
    }
}
