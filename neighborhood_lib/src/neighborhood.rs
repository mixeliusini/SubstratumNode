// Copyright (c) 2017-2018, Substratum LLC (https://substratum.net) and/or its affiliates. All rights reserved.
use actix::Actor;
use actix::Addr;
use actix::Context;
use actix::Handler;
use actix::Syn;
use sub_lib::dispatcher::Component;
use sub_lib::node_addr::NodeAddr;
use sub_lib::route::Route;
use sub_lib::cryptde::Key;
use sub_lib::neighborhood::NeighborhoodSubs;
use sub_lib::peer_actors::BindMessage;
use sub_lib::cryptde::CryptDE;
use sub_lib::neighborhood::NodeQueryMessage;
use sub_lib::neighborhood::NodeDescriptor;
use actix::MessageResult;
use sub_lib::neighborhood::RouteQueryMessage;
use sub_lib::hopper::ExpiredCoresPackage;
use sub_lib::route::RouteSegment;

pub struct Neighborhood {
    cryptde: &'static CryptDE,
    neighboring_nodes: Vec<NodeDescriptor>,
}

impl Actor for Neighborhood {
    type Context = Context<Self>;
}

impl Handler<BindMessage> for Neighborhood {
    type Result = ();

    fn handle(&mut self, _msg: BindMessage, _ctx: &mut Self::Context) -> Self::Result {
        ()
    }
}

impl Handler<NodeQueryMessage> for Neighborhood {
    type Result = MessageResult<NodeQueryMessage>;

    fn handle(&mut self, msg: NodeQueryMessage, _ctx: &mut Self::Context) -> <Self as Handler<NodeQueryMessage>>::Result {
        let result_opt = self.neighboring_nodes.iter()
            .find(|node_ref_ref| {
                self.matches(node_ref_ref, &msg)
            })
            .map(|r| r.clone());

        MessageResult(result_opt)
    }
}

impl Handler<RouteQueryMessage> for Neighborhood {
    type Result = MessageResult<RouteQueryMessage>;

    fn handle(&mut self, msg: RouteQueryMessage, _ctx: &mut Self::Context) -> <Self as Handler<RouteQueryMessage>>::Result {
        if msg.minimum_hop_count == 0 {
            let route = Route::new(vec! (
                RouteSegment::new(vec! (&self.cryptde.public_key(), &self.cryptde.public_key ()), Component::ProxyClient),
                RouteSegment::new(vec! (&self.cryptde.public_key(), &self.cryptde.public_key()), Component::ProxyServer)
            ), self.cryptde).expect("Couldn't create route");
            return MessageResult(Some(route));
        }
        MessageResult(None)
    }
}

impl Handler<ExpiredCoresPackage> for Neighborhood {
    type Result = ();

    fn handle(&mut self, _msg: ExpiredCoresPackage, _ctx: &mut Self::Context) -> Self::Result {
        ()
    }
}

impl Neighborhood {
    pub fn new(cryptde: &'static CryptDE, config: Vec<(Key, NodeAddr)>) -> Self {
        Neighborhood {
            cryptde,
            neighboring_nodes: config.into_iter().map(|(key, node_addr)| {
                NodeDescriptor::new (key, Some (node_addr))
            }).collect ()
        }
    }

    pub fn make_subs_from(addr: &Addr<Syn, Neighborhood>) -> NeighborhoodSubs {
        NeighborhoodSubs {
            bind: addr.clone ().recipient::<BindMessage>(),
            node_query: addr.clone ().recipient::<NodeQueryMessage>(),
            route_query: addr.clone ().recipient::<RouteQueryMessage>(),
            from_hopper: addr.clone ().recipient::<ExpiredCoresPackage>(),
        }
    }

    // TODO: Turn these into actor messages
    // crashpoint - unused so far
    #[allow (dead_code)]
    fn route_one_way(&self, _remote_recipient: Component) -> Result<(Route, Key), ()> {
        unimplemented!()
    }

    // crashpoint - unused so far
    #[allow (dead_code)]
    fn route_round_trip(&self, _remote_recipient: Component, _local_recipient: Component) -> Result<(Route, Key), ()> {
        unimplemented!()
    }

    fn matches (&self, node_ref_ref: &&NodeDescriptor, query: &NodeQueryMessage) -> bool {
        match query {
            NodeQueryMessage::PublicKey (ref public_key) => public_key == &node_ref_ref.public_key,
            NodeQueryMessage::IpAddress (ref ip_address) => match node_ref_ref.node_addr_opt  {
                None => false,
                Some(ref node_addr) => ip_address == &node_addr.ip_addr()
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use std::net::IpAddr;
    use actix::Arbiter;
    use actix::Recipient;
    use actix::System;
    use actix::msgs;
    use futures::future::Future;
    use test_utils::test_utils::cryptde;


    #[test]
    fn responds_with_none_when_initially_configured_with_no_data () {
        let cryptde = cryptde ();
        let system = System::new ("responds_with_none_when_initially_configured_with_no_data");
        let subject = Neighborhood::new (cryptde, vec! ());
        let addr: Addr<Syn, Neighborhood> = subject.start ();
        let sub: Recipient<Syn, NodeQueryMessage> = addr.recipient::<NodeQueryMessage> ();

        let future = sub.send(NodeQueryMessage::PublicKey (Key::new (&b"booga"[..])));

        Arbiter::system().try_send(msgs::SystemExit(0)).unwrap ();
        system.run ();
        let result = future.wait ().unwrap ();
        assert_eq! (result.is_none (), true);
    }

    #[test]
    fn responds_with_none_when_key_query_matches_no_configured_data () {
        let cryptde = cryptde ();
        let system = System::new ("responds_with_none_when_initially_configured_with_no_data");
        let subject = Neighborhood::new (cryptde, vec! (
            (Key::new (&b"booga"[..]), NodeAddr::new (&IpAddr::from_str ("1.2.3.4").unwrap(), &vec! (1234, 2345))),
        ));
        let addr: Addr<Syn, Neighborhood> = subject.start ();
        let sub: Recipient<Syn, NodeQueryMessage> = addr.recipient::<NodeQueryMessage> ();

        let future = sub.send(NodeQueryMessage::PublicKey (Key::new (&b"blah"[..])));

        Arbiter::system().try_send(msgs::SystemExit(0)).unwrap ();
        system.run ();
        let result = future.wait ().unwrap ();
        assert_eq! (result.is_none (), true);
    }

    #[test]
    fn responds_with_result_when_key_query_matches_configured_data () {
        let cryptde = cryptde ();
        let system = System::new ("responds_with_none_when_initially_configured_with_no_data");
        let public_key = Key::new (&b"booga"[..]);
        let node_addr = NodeAddr::new(&IpAddr::from_str("1.2.3.4").unwrap(), &vec!(1234, 2345));
        let another_node_addr = NodeAddr::new(&IpAddr::from_str("2.3.4.5").unwrap(), &vec!(1234, 2345));
        let subject = Neighborhood::new (cryptde, vec! (
            (public_key.clone (), node_addr.clone ()),
            (public_key.clone (), another_node_addr.clone ()),
        ));
        let addr: Addr<Syn, Neighborhood> = subject.start ();
        let sub: Recipient<Syn, NodeQueryMessage> = addr.recipient::<NodeQueryMessage> ();

        let future = sub.send(NodeQueryMessage::PublicKey (Key::new (&b"booga"[..])));

        Arbiter::system().try_send(msgs::SystemExit(0)).unwrap ();
        system.run ();
        let result = future.wait ().unwrap ();
        assert_eq! (result.unwrap (), NodeDescriptor::new (public_key, Some (node_addr)));
    }

    #[test]
    fn responds_with_none_when_ip_address_query_matches_no_configured_data () {
        let cryptde = cryptde ();
        let system = System::new ("responds_with_none_when_initially_configured_with_no_data");
        let subject = Neighborhood::new (cryptde, vec! (
            (Key::new (&b"booga"[..]), NodeAddr::new (&IpAddr::from_str ("1.2.3.4").unwrap(), &vec! (1234, 2345))),
        ));
        let addr: Addr<Syn, Neighborhood> = subject.start ();
        let sub: Recipient<Syn, NodeQueryMessage> = addr.recipient::<NodeQueryMessage> ();

        let future = sub.send(NodeQueryMessage::IpAddress (IpAddr::from_str("2.3.4.5").unwrap()));

        Arbiter::system().try_send(msgs::SystemExit(0)).unwrap ();
        system.run ();
        let result = future.wait ().unwrap ();
        assert_eq! (result.is_none (), true);
    }

    #[test]
    fn responds_with_result_when_ip_address_query_matches_configured_data () {
        let cryptde = cryptde ();
        let system = System::new ("responds_with_none_when_initially_configured_with_no_data");
        let public_key = Key::new (&b"booga"[..]);
        let another_public_key = Key::new (&b"gooba"[..]);
        let node_addr = NodeAddr::new(&IpAddr::from_str("1.2.3.4").unwrap(), &vec!(1234, 2345));
        let subject = Neighborhood::new (cryptde, vec! (
            (public_key.clone (), node_addr.clone ()),
            (another_public_key.clone (), node_addr.clone ()),
        ));
        let addr: Addr<Syn, Neighborhood> = subject.start ();
        let sub: Recipient<Syn, NodeQueryMessage> = addr.recipient::<NodeQueryMessage> ();

        let future = sub.send(NodeQueryMessage::IpAddress (IpAddr::from_str("1.2.3.4").unwrap()));

        Arbiter::system().try_send(msgs::SystemExit(0)).unwrap ();
        system.run ();
        let result = future.wait ().unwrap ();
        assert_eq! (result.unwrap (), NodeDescriptor::new (public_key, Some (node_addr)));
    }

    #[test]
    fn responds_with_none_when_asked_for_route_with_too_many_hops () {
        let cryptde = cryptde ();
        let system = System::new ("responds_with_none_when_asked_for_route_with_empty_database");
        let subject = Neighborhood::new (cryptde, vec! ());
        let addr: Addr<Syn, Neighborhood> = subject.start ();
        let sub: Recipient<Syn, RouteQueryMessage> = addr.recipient::<RouteQueryMessage> ();

        let future = sub.send (RouteQueryMessage {minimum_hop_count: 5});

        Arbiter::system().try_send(msgs::SystemExit(0)).unwrap ();
        system.run ();
        let result = future.wait ().unwrap ();
        assert_eq! (result, None);
    }

    #[test]
    fn responds_with_standard_zero_hop_route_when_requested () {
        let cryptde = cryptde ();
        let system = System::new ("responds_with_standard_zero_hop_route_when_requested");
        let subject = Neighborhood::new (cryptde, vec! ());
        let addr: Addr<Syn, Neighborhood> = subject.start ();
        let sub: Recipient<Syn, RouteQueryMessage> = addr.recipient::<RouteQueryMessage> ();

        let future = sub.send (RouteQueryMessage {minimum_hop_count: 0});

        Arbiter::system().try_send(msgs::SystemExit(0)).unwrap ();
        system.run ();
        let result = future.wait ().unwrap ().unwrap ();
        let expected_route = Route::new(vec! (
            RouteSegment::new(vec! (&cryptde.public_key(), &cryptde.public_key()), Component::ProxyClient),
            RouteSegment::new(vec! (&cryptde.public_key(), &cryptde.public_key()), Component::ProxyServer)
        ), cryptde).unwrap ();
        assert_eq! (result, expected_route);
    }
}
