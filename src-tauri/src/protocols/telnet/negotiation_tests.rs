use super::*;
use NegotiationState::*;
use NegotiationVerb::*;
use OptionDirection::*;

const OPTION: u8 = 42;
const DIRECTIONS: [(
    OptionDirection,
    NegotiationVerb,
    NegotiationVerb,
    NegotiationVerb,
    NegotiationVerb,
); 2] = [
    (Local, Do, Dont, Will, Wont),
    (Remote, Will, Wont, Do, Dont),
];

fn command(verb: NegotiationVerb) -> Option<NegotiationCommand> {
    Some(NegotiationCommand {
        verb,
        option: OPTION,
    })
}

// Reach each public Q state through real requests/acknowledgments, without
// injecting private state. Transition expectations below come from RFC 1143 §7.
fn at_state(
    direction: OptionDirection,
    positive: NegotiationVerb,
    target: NegotiationState,
) -> TelnetNegotiator {
    let mut negotiator = TelnetNegotiator::new(OptionPolicy::new(&[OPTION], &[OPTION]));
    match target {
        No => {}
        Yes | WantNo | WantNoOpposite => {
            negotiator.receive(positive, OPTION);
            if target != Yes {
                negotiator.request(direction, OPTION, false).unwrap();
            }
            if target == WantNoOpposite {
                negotiator.request(direction, OPTION, true).unwrap();
            }
        }
        WantYes | WantYesOpposite => {
            negotiator.request(direction, OPTION, true).unwrap();
            if target == WantYesOpposite {
                negotiator.request(direction, OPTION, false).unwrap();
            }
        }
    }
    assert_eq!(negotiator.state(direction, OPTION), target);
    assert_eq!(negotiator.is_enabled(direction, OPTION), target == Yes);
    negotiator
}

#[test]
fn every_received_q_transition_matches_the_reference_in_both_directions() {
    // (before, receive positive?, after, send positive?)
    let cases = [
        (No, true, Yes, Some(true)),
        (No, false, No, None),
        (Yes, true, Yes, None),
        (Yes, false, No, Some(false)),
        (WantNo, true, No, None),
        (WantNo, false, No, None),
        (WantNoOpposite, true, Yes, None),
        (WantNoOpposite, false, WantYes, Some(true)),
        (WantYes, true, Yes, None),
        (WantYes, false, No, None),
        (WantYesOpposite, true, WantNo, Some(false)),
        (WantYesOpposite, false, No, None),
    ];
    for (direction, positive, negative, enable, disable) in DIRECTIONS {
        for (before, received, after, sent) in cases {
            let mut negotiator = at_state(direction, positive, before);
            let result = negotiator.receive(if received { positive } else { negative }, OPTION);
            assert_eq!(
                result,
                sent.and_then(|value| command(if value { enable } else { disable })),
                "{direction:?} {before:?} positive={received}"
            );
            assert_eq!(negotiator.state(direction, OPTION), after);
            assert_eq!(negotiator.is_enabled(direction, OPTION), after == Yes);
        }
    }
}

#[test]
fn every_requested_q_transition_including_duplicates_and_reversals() {
    let cases = [
        (No, true, WantYes, Some(true)),
        (No, false, No, None),
        (Yes, true, Yes, None),
        (Yes, false, WantNo, Some(false)),
        (WantNo, true, WantNoOpposite, None),
        (WantNo, false, WantNo, None),
        (WantNoOpposite, true, WantNoOpposite, None),
        (WantNoOpposite, false, WantNo, None),
        (WantYes, true, WantYes, None),
        (WantYes, false, WantYesOpposite, None),
        (WantYesOpposite, true, WantYes, None),
        (WantYesOpposite, false, WantYesOpposite, None),
    ];
    for (direction, positive, _, enable, disable) in DIRECTIONS {
        for (before, requested, after, sent) in cases {
            let mut negotiator = at_state(direction, positive, before);
            assert_eq!(
                negotiator.request(direction, OPTION, requested),
                Ok(sent.and_then(|value| command(if value { enable } else { disable }))),
                "{direction:?} {before:?} enable={requested}"
            );
            assert_eq!(negotiator.state(direction, OPTION), after);
            assert_eq!(negotiator.is_enabled(direction, OPTION), after == Yes);
        }
    }
}

#[test]
fn default_policy_denies_every_option_without_reply_loops() {
    let mut negotiator = TelnetNegotiator::default();
    for option in 0..=255 {
        for (direction, positive, negative, _, refusal) in DIRECTIONS {
            assert_eq!(
                negotiator.request(direction, option, true),
                Err(NegotiationError::OptionNotAllowed { direction, option })
            );
            assert_eq!(negotiator.state(direction, option), No);
            assert_eq!(negotiator.request(direction, option, false), Ok(None));
            for _ in 0..2 {
                assert_eq!(
                    negotiator.receive(positive, option),
                    Some(NegotiationCommand {
                        verb: refusal,
                        option
                    })
                );
                assert_eq!(negotiator.receive(negative, option), None);
                assert_eq!(negotiator.state(direction, option), No);
            }
        }
    }
}

#[test]
fn allowlists_are_directional_cover_all_codes_and_ignore_duplicates() {
    for option in 0..=255 {
        for (direction, positive, _, enable, _) in DIRECTIONS {
            let policy = match direction {
                Local => OptionPolicy::new(&[option, option], &[]),
                Remote => OptionPolicy::new(&[], &[option, option]),
            };
            let mut negotiator = TelnetNegotiator::new(policy.clone());
            for other_option in 0..=255 {
                for other_direction in [Local, Remote] {
                    assert_eq!(
                        policy.allows(other_direction, other_option),
                        other_direction == direction && other_option == option
                    );
                }
            }
            assert_eq!(
                negotiator.receive(positive, option),
                Some(NegotiationCommand {
                    verb: enable,
                    option
                })
            );
            assert!(negotiator.is_enabled(direction, option));
        }
    }
}

#[test]
fn reset_clears_all_q_states_and_preserves_policy() {
    for (direction, positive, _, enable, _) in DIRECTIONS {
        for state in [No, Yes, WantNo, WantNoOpposite, WantYes, WantYesOpposite] {
            let mut negotiator = at_state(direction, positive, state);
            negotiator.reset();
            for option in 0..=255 {
                for side in [Local, Remote] {
                    assert_eq!(negotiator.state(side, option), No);
                }
            }
            assert_eq!(
                negotiator.request(direction, OPTION, true),
                Ok(command(enable))
            );
            assert!(matches!(
                negotiator.request(direction, OPTION + 1, true),
                Err(NegotiationError::OptionNotAllowed { .. })
            ));
        }
    }
}

#[test]
fn option_direction_and_instance_state_are_isolated_even_after_policy_errors() {
    let policy = OptionPolicy::new(&[42, 43], &[42]);
    let mut first = TelnetNegotiator::new(policy.clone());
    let second = TelnetNegotiator::new(policy);
    first.request(Local, 42, true).unwrap();
    first.request(Local, 42, false).unwrap();
    first.receive(Will, 42);
    first.receive(Do, 43);
    assert!(first.request(Remote, 43, true).is_err());
    assert_eq!(first.state(Local, 42), WantYesOpposite);
    assert_eq!(first.state(Remote, 42), Yes);
    assert_eq!(first.state(Local, 43), Yes);
    assert_eq!(first.state(Remote, 43), No);
    first.reset();
    for option in 0..=255 {
        for direction in [Local, Remote] {
            assert_eq!(second.state(direction, option), No);
        }
    }
}

#[test]
fn commands_convert_to_existing_events_without_changing_fields() {
    for verb in [Will, Wont, Do, Dont] {
        assert_eq!(
            TelnetEvent::from(NegotiationCommand { verb, option: 255 }),
            TelnetEvent::Negotiation { verb, option: 255 }
        );
    }
}
