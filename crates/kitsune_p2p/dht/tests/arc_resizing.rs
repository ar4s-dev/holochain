//! Tests of arq resizing behavior.

#![cfg(feature = "testing")]

mod common;

use kitsune_p2p_dht::arq::*;
use kitsune_p2p_dht::op::*;
use kitsune_p2p_dht_arc::ArcInterval;

use kitsune_p2p_dht::test_utils::generate_ideal_coverage;
use kitsune_p2p_dht::test_utils::seeded_rng;

fn resize_to_equilibrium(view: &PeerView, arq: &mut Arq) {
    while view.update_arq(arq) {}
}

#[test]
/// If extrapolated coverage remains above the maximum coverage threshold even
/// when shrinking towards empty, let the arq be resized as small as possible
/// before losing peers.
fn test_shrink_towards_empty() {
    let mut rng = seeded_rng(None);

    // aim for coverage between 10 and 12
    let strat = ArqStrat {
        min_coverage: 10.0,
        buffer: 0.2,
        max_power_diff: 2,
        ..Default::default()
    };
    let jitter = 0.01;

    // generate peers with a bit too much coverage (14 > 12)
    let peers: Vec<_> = generate_ideal_coverage(&mut rng, &strat, Some(14.5), 100, jitter, 0);
    let peer_power = peers.iter().map(|p| p.power()).min().unwrap();
    let view = PeerView::new(strat.clone(), peers);

    // start with a full arq at max power
    let mut arq = Arq::new_full(0.into(), strat.max_power);
    resize_to_equilibrium(&view, &mut arq);
    // test that the arc gets reduced in power to match those of its peers
    assert!(
        arq.power() <= peer_power,
        "{} <= {}",
        arq.power(),
        peer_power
    );
}

#[test]
/// If extrapolated coverage remains below the minimum coverage threshold even
/// when growing to full, let the arq be resized as large as it can be under
/// the constraints of the ArqStrat.
fn test_grow_towards_full() {
    let mut rng = seeded_rng(None);

    // aim for coverage between 10 and 12, with no limit on power diff
    let strat = ArqStrat {
        min_coverage: 10.0,
        buffer: 0.2,
        max_power_diff: 2,
        ..Default::default()
    };
    strat.max_chunks();
    let jitter = 0.01;

    // generate peers with deficient coverage
    let peers: Vec<_> = generate_ideal_coverage(&mut rng, &strat, Some(7.0), 1000, jitter, 0);
    let peer_power = peers.iter().map(|p| p.power()).min().unwrap();
    let view = PeerView::new(strat.clone(), peers);

    // start with an arq comparable to one's peers
    let mut arq = Arq::new(0.into(), peer_power, 12);
    resize_to_equilibrium(&view, &mut arq);
    // ensure that the arq grows to full size
    assert_eq!(arq.power(), peer_power + 2);
    assert_eq!(arq.count(), strat.max_chunks());
}

#[test]
/// If extrapolated coverage remains below the minimum coverage threshold even
/// when growing to full, let the arq be resized to full when the max_power_diff
/// is not a constraint
fn test_grow_to_full() {
    let mut rng = seeded_rng(None);

    // aim for coverage between 10 and 12, with no limit on power diff
    let strat = ArqStrat {
        min_coverage: 10.0,
        buffer: 0.2,
        max_power_diff: 32,
        ..Default::default()
    };
    let jitter = 0.01;
    dbg!(strat.max_chunks());

    // generate peers with deficient coverage
    let peers: Vec<_> = generate_ideal_coverage(&mut rng, &strat, Some(7.0), 1000, jitter, 0);
    let peer_power = peers.iter().map(|p| p.power()).min().unwrap();
    let view = PeerView::new(strat.clone(), peers);

    // start with an arq comparable to one's peers
    let mut arq = Arq::new(0.into(), peer_power, 12);
    print_arq(&arq, 64);
    while view.update_arq(&mut arq) {
        print_arq(&arq, 64);
    }
    // ensure that the arq grows to full size
    assert_eq!(arq.power(), strat.max_power);
    assert_eq!(arq.count(), 8);
    assert!(is_full(arq.power(), arq.count()));
}

#[test]
#[ignore = "this may not be a property we want"]
// XXX: We only want to do this if other peers have not moved. But currently
//      we have no way of determining this.
//
/// If the current coverage is far from the target, shrinking can occur in
/// multiple chunks
fn test_shrink_by_multiple_chunks() {
    let mut rng = seeded_rng(None);

    // aim for coverage between 10 and 12
    let strat = ArqStrat {
        min_coverage: 10.0,
        buffer: 0.2,
        ..Default::default()
    };
    let jitter = 0.01;

    // generate peers with far too much coverage
    let peers: Vec<_> = generate_ideal_coverage(&mut rng, &strat, Some(22.0), 1000, jitter, 0);
    let peer_power = peers.iter().map(|p| p.power()).min().unwrap();
    let view = PeerView::new(strat.clone(), peers);

    let arq = Arq::new(0.into(), peer_power + 1, 12);
    let mut resized = arq.clone();
    view.update_arq(&mut resized);
    assert_eq!(arq.power(), resized.power());
    assert_eq!(resized.count(), 6);
}

#[test]
#[ignore = "this may not be a property we want"]
// XXX: We only want to do this if other peers have not moved. But currently
//      we have no way of determining this.
//
/// If the current coverage is far from the target, growing can occur in
/// multiple chunks
fn test_grow_by_multiple_chunks() {
    let mut rng = seeded_rng(None);

    // aim for coverage between 10 and 12
    let strat = ArqStrat {
        min_coverage: 10.0,
        buffer: 0.2,
        ..Default::default()
    };
    let jitter = 0.01;

    // generate peers with far too little coverage
    let peers: Vec<_> = generate_ideal_coverage(&mut rng, &strat, Some(5.0), 1000, jitter, 0);
    let peer_power = peers.iter().map(|p| p.power()).min().unwrap();
    let view = PeerView::new(strat.clone(), peers);

    let arq = Arq::new(0.into(), peer_power - 1, 6);
    let mut resized = arq.clone();
    view.update_arq(&mut resized);
    assert_eq!(arq.power(), resized.power());
    assert_eq!(resized.count(), 12);
}

#[test]
/// If the space to our left is oversaturated by double,
/// and the space to our right is completely empty,
/// we should not resize
///
/// (not a very good test, probably)
fn test_degenerate_asymmetrical_coverage() {
    let other = ArqBounds::from_interval(4, ArcInterval::new(0x0, 0x80))
        .unwrap()
        .to_arq();
    let others = vec![other; 10];
    // aim for coverage between 5 and 6.
    let strat = ArqStrat {
        min_coverage: 5.0,
        buffer: 0.1,
        ..Default::default()
    };
    let view = PeerView::new(strat, others);

    let arq = Arq::new(
        Loc::from(0x100 / 2),
        4, // log2 of 0x10
        0x10,
    );
    assert_eq!(arq.to_interval(), ArcInterval::new(0, 0x100 - 1));

    let extrapolated = view.extrapolated_coverage(&arq.to_bounds());
    assert_eq!(extrapolated, 5.0);
    let old = arq.clone();
    let mut new = arq.clone();
    let resized = view.update_arq(&mut new);
    assert_eq!(old, new);
    assert!(!resized);
}

#[test]
/// Test resizing across several quantization levels to get a feel for how
/// it should work.
fn test_scenario() {
    let mut rng = seeded_rng(None);

    // aim for coverage between 10 and 12.
    let strat = ArqStrat {
        min_coverage: 10.0,
        buffer: 0.2,
        max_power_diff: 2,
        ..Default::default()
    };
    let jitter = 0.000;

    {
        // start with a full arq
        let mut arq = Arq::new_full(Loc::from(0x0), strat.max_power);
        // create 10 peers, all with full arcs, fully covering the DHT
        let peers: Vec<_> = generate_ideal_coverage(&mut rng, &strat, None, 10, jitter, 0);
        let view = PeerView::new(strat.clone(), peers);
        let extrapolated = view.extrapolated_coverage(&arq.to_bounds());
        assert_eq!(extrapolated, 10.0);

        // expect that the arq remains full under these conditions
        let resized = view.update_arq(&mut arq);
        assert!(!resized);
    }

    {
        // start with a full arq again
        let mut arq = Arq::new_full(Loc::from(0x0), strat.max_power);
        // create 100 peers, with arcs at about 10%,
        // covering a bit more than they need to
        let peers = generate_ideal_coverage(&mut rng, &strat, Some(13.0), 100, jitter, 0);

        {
            let peer_power = peers.iter().map(|p| p.power()).min().unwrap();
            // print_arqs(&peers, 64);
            assert_eq!(peer_power, 26);

            let view = PeerView::new(strat.clone(), peers.clone());
            let extrapolated = view.extrapolated_coverage(&arq.to_bounds());
            assert!(extrapolated > strat.max_coverage());
            // assert!(strat.min_coverage <= extrapolated && extrapolated <= strat.max_coverage());

            // update the arq until there is no change
            while view.update_arq(&mut arq) {}

            // expect that the arq shrinks
            assert_eq!(arq.power(), peer_power);
            assert!(arq.count() <= 8);
        }
        {
            // create the same view but with all arcs cut in half, so that the
            // coverage is uniformly undersaturated.
            let peers: Vec<_> = peers
                .clone()
                .iter_mut()
                .map(|arq| {
                    let mut arq = arq.downshift();
                    *arq.count_mut() = arq.count() / 2;
                    arq
                })
                .collect();
            let peer_power = peers.iter().map(|p| p.power()).min().unwrap();
            let view = PeerView::new(strat.clone(), peers);
            print_arq(&arq, 64);
            // assert that our arc will grow as large as it can to pick up the slack.
            while view.update_arq(&mut arq) {
                print_arq(&arq, 64);
            }
            assert_eq!(arq.power(), peer_power + strat.max_power_diff);
            assert!(arq.count() == strat.max_chunks());
        }
    }
}
