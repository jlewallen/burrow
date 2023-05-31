use tracing::*;

use super::Message;

#[derive(Debug)]
pub struct Sender<S> {
    pub queue: Vec<S>,
}

impl<S> Default for Sender<S> {
    fn default() -> Self {
        Self {
            queue: Default::default(),
        }
    }
}

#[allow(dead_code)]
impl<S> Sender<S>
where
    S: std::fmt::Debug,
{
    pub fn send(&mut self, message: S) -> anyhow::Result<()> {
        debug!("Sending {:?}", &message);
        self.queue.push(message);

        Ok(())
    }

    pub fn iter(&self) -> impl Iterator<Item = &S> {
        self.queue.iter()
    }

    pub fn clear(&mut self) {
        self.queue.clear()
    }

    pub fn pop(&mut self) -> Option<S> {
        self.queue.pop()
    }
}

impl<B> Sender<Message<B>> {
    #[cfg(test)]
    pub fn bodies(&self) -> impl Iterator<Item = &B> {
        self.queue.iter().map(|m| &m.body)
    }
}

pub enum Transition<S, M> {
    None,
    Direct(S),
    Send(M, S),
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
                debug!("(send) {:?}", &sending);
                sender.send(sending)?;
                debug!("(send) {:?} -> {:?}", &self.state, &state);
                self.state = state;
                Ok(())
            }
            Transition::SendOnly(sending) => {
                debug!("(send-only) {:?}", &sending);
                sender.send(sending)?;
                Ok(())
            }
        }
    }
}
