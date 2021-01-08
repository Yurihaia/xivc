use std::collections::HashMap;

use xivc::{
    job::gnb::{GnbEventHandler, GnbJobState},
    math::{
        ActionStat, CDHHandle, EotSnapshot, PlayerInfo, PlayerStats, SpeedStat, WeaponInfo, XivMath,
    },
    sim::{
        ActionError, Actor, ActorId, CastEvent, DamageEvent, DamageInstance, EffectApplyEvent,
        EffectInstance, EffectRemoveEvent, Runtime, StatusEffect,
    },
    Clan, DamageElement, DamageType, Job,
};

use xivc::job::gnb::GnbAction;

struct Centis(pub u32);
impl std::fmt::Display for Centis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{:02}", self.0 / 100, self.0 % 100)
    }
}

#[allow(dead_code)]
#[derive(Debug)]
enum Event {
    Damage(DamageEvent),
    EffectApply(EffectApplyEvent),
    EffectRemove(EffectRemoveEvent),
    Cast(CastEvent<GnbAction>),
    DotTrigger,
}

#[derive(Clone, Debug)]
struct DotSnapshot {
    stats: EotSnapshot,
    el: DamageElement,
    ty: DamageType,
    source: Vec<EffectInstance>,
    target: Vec<EffectInstance>,
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
struct DotEffect {
    target: ActorId,
    effect: StatusEffect,
}

struct GnbEventProxy<'r> {
    rt: &'r mut Runtime<Event>,
    targets: &'r Vec<ActorId>,
    source: ActorId,
    stats: &'r XivMath,
    dots: &'r mut HashMap<DotEffect, DotSnapshot>,
}

impl<'r> GnbEventHandler for GnbEventProxy<'r> {
    fn damage(&mut self, potency: u64) {
        for target_id in self.targets.iter().copied() {
            let source = self.rt.get_actor(self.source).unwrap();
            let target = self.rt.get_actor(target_id).unwrap();
            let stats = source.effects.iter().chain(target.effects.iter()).fold(
                self.stats.stats,
                |c, x| {
                    if let Some(f) = x.effect.stats {
                        f(*x, c)
                    } else {
                        c
                    }
                },
            );
            let stats = XivMath::new(stats, self.stats.weapon, self.stats.info);
            let crit = source
                .effects
                .iter()
                .filter_map(|x| Some((x, x.effect.crit.outgoing?)))
                .chain(
                    target
                        .effects
                        .iter()
                        .filter_map(|x| Some((x, x.effect.crit.incoming?))),
                )
                .fold(stats.crt_chance(), |c, (x, f)| f(*x, c));
            let dhit = source
                .effects
                .iter()
                .filter_map(|x| Some((x, x.effect.dhit.outgoing?)))
                .chain(
                    target
                        .effects
                        .iter()
                        .filter_map(|x| Some((x, x.effect.dhit.outgoing?))),
                )
                .fold(stats.dh_chance(), |c, (x, f)| f(*x, c));
            let damage = DamageInstance {
                dmg: stats.prebuff_action_damage(
                    potency,
                    ActionStat::AttackPower,
                    100,
                    CDHHandle::Avg {
                        chance: crit as u16,
                    },
                    CDHHandle::Avg {
                        chance: dhit as u16,
                    },
                    10000,
                ),
                el: DamageElement::None,
                ty: DamageType::Slashing,
            };
            let damage = source
                .effects
                .iter()
                .filter_map(|x| Some((x, x.effect.damage.outgoing?)))
                .chain(
                    target
                        .effects
                        .iter()
                        .filter_map(|x| Some((x, x.effect.damage.outgoing?))),
                )
                .fold(damage, |c, (x, f)| f(*x, c));
            self.rt.add_event(
                Event::Damage(DamageEvent {
                    damage,
                    source: self.source,
                    target: target_id,
                }),
                60,
            );
            println!("Damage queued: {}", damage.dmg);
        }
    }

    fn effect_apply(&mut self, effect: EffectInstance) {
        for target in self.targets.iter().copied() {
            self.rt.add_event(
                Event::EffectApply(EffectApplyEvent {
                    effect,
                    source: self.source,
                    target,
                }),
                // Delay by a bit just to make things more consistent
                60,
            );
        }
    }

    fn dot_apply(&mut self, effect: EffectInstance, dot_potency: u64) {
        self.effect_apply(effect);
        for target_id in self.targets.iter().copied() {
            let source = self.rt.get_actor(self.source).unwrap();
            let target = self.rt.get_actor(target_id).unwrap();
            let stats = source.effects.iter().chain(target.effects.iter()).fold(
                self.stats.stats,
                |c, x| {
                    if let Some(f) = x.effect.stats {
                        f(*x, c)
                    } else {
                        c
                    }
                },
            );
            let stats = XivMath::new(stats, self.stats.weapon, self.stats.info);
            let crit = source
                .effects
                .iter()
                .filter_map(|x| Some((x, x.effect.crit.outgoing?)))
                .chain(
                    target
                        .effects
                        .iter()
                        .filter_map(|x| Some((x, x.effect.crit.incoming?))),
                )
                .fold(stats.crt_chance(), |c, (x, f)| f(*x, c));
            let dhit = source
                .effects
                .iter()
                .filter_map(|x| Some((x, x.effect.dhit.outgoing?)))
                .chain(
                    target
                        .effects
                        .iter()
                        .filter_map(|x| Some((x, x.effect.dhit.outgoing?))),
                )
                .fold(stats.dh_chance(), |c, (x, f)| f(*x, c));
            let snapshot = stats.dot_damage_snapshot(
                dot_potency,
                ActionStat::AttackPower,
                100,
                SpeedStat::SkillSpeed,
                crit as u16,
                dhit as u16,
            );
            let source_damage_buffs = source
                .effects
                .iter()
                .filter(|v| v.effect.damage.outgoing.is_some())
                .copied()
                .collect();
            let target_damage_debuffs = target
                .effects
                .iter()
                .filter(|v| v.effect.damage.incoming.is_some())
                .copied()
                .collect();
            self.dots.insert(
                DotEffect {
                    effect: effect.effect,
                    target: target_id,
                },
                DotSnapshot {
                    stats: snapshot,
                    source: source_damage_buffs,
                    target: target_damage_debuffs,
                    el: DamageElement::None,
                    ty: DamageType::Slashing,
                },
            );
        }
    }
}

#[allow(dead_code)]
mod buffs {
    use xivc::{sim::StatusEffect, status_effect};

    pub const CHAIN: StatusEffect = status_effect!(
        "Chain Strategem" { crit { in = 10 } }
    );

    pub const LITANY: StatusEffect = status_effect!(
        "Battle Litany" { crit { out = 10 } }
    );

    pub const DIV: StatusEffect = status_effect!(
        "Divination" { damage { out = 106 / 100 }}
    );

    pub const TRICK: StatusEffect = status_effect!(
        "Trick Attack" { damage { in = 105 / 100 }}
    );

    pub const TECH: StatusEffect = status_effect!(
        "Technical Finish" { damage { out = 105 / 100 }}
    );

    pub const DEVOTION: StatusEffect = status_effect!(
        "Devotion" { damage { out = 105 / 100 }}
    );

    pub const EMBOLDEN: StatusEffect = status_effect!(
        "Embolden" { damage { out = |e, mut d| {
            let m = e.time as u64 / 400 + if e.time as u64 % 400 == 0 {
                0
            } else {
                100
            };
            d.dmg = d.dmg * m / 100;
            d
        }}}
    );

    pub const BROTHERHOOD: StatusEffect = status_effect!(
        "Brotherhood" { damage { out = 105 / 100 }}
    );

    pub const POT: StatusEffect = status_effect!(
        "Grade 4 Tincture of Strength" {
            stats {
                |_, mut s| {
                    s.str += (s.str / 10).min(464);
                    s
                }
            }
        }
    );
}

fn main() {
    let mut rt = Runtime::<Event>::new();

    let boss = rt.add_actor(Actor {
        name: "Dummy Boss",
        health: 1000000,
        effects: Vec::new(),
        mirrors: Vec::new(),
    });

    let player = rt.add_actor(Actor {
        name: ":qwestgnb:",
        health: 200000,
        effects: Vec::new(),
        mirrors: Vec::new(),
    });
    let pstats = XivMath::new(
        PlayerStats {
            str: 5784,
            crt: 4148,
            dh: 1400,
            det: 2454,
            ten: 832,
            sks: 1155,
            ..PlayerStats::default(80)
        },
        WeaponInfo {
            phys_dmg: 134,
            magic_dmg: 0,
            delay: 280,
            auto: 12506,
        },
        PlayerInfo {
            clan: Clan::Xaela,
            job: Job::GNB,
            lvl: 80,
        },
    );
    let mut jobstate = GnbJobState::new();

    let mut add_event = |eff, dur, del: &[u32], target| {
        for x in del {
            rt.add_event(
                Event::EffectApply(EffectApplyEvent {
                    effect: EffectInstance::new(eff, dur, 1),
                    source: player,
                    target,
                }),
                *x,
            );
        }
    };

    add_event(buffs::TECH, 2000, &[750], player);
    add_event(buffs::EMBOLDEN, 2000, &[750], player);
    add_event(buffs::TRICK, 1500, &[1000], boss);
    add_event(buffs::CHAIN, 1500, &[1000], boss);
    add_event(buffs::DEVOTION, 1500, &[1000], player);
    add_event(buffs::DIV, 1500, &[1000], player);

    // DoT Implementation
    // God this is so bad I really should just bite the bullet
    // and assume all damaging debuffs are just % multipliers
    let mut dots: HashMap<DotEffect, DotSnapshot> = HashMap::new();

    let mut rotation: Vec<GnbAction> = {
        use GnbAction::*;
        vec![
            Lightning,
            Keen, Bloodfest, NoMercy,
            Brutal,
            Gnashing,
            Jugular,
            Burst,
            Divide,
            Sonic,
            Blasting,
            Shock,
            Savage,
            Divide,
            Abdomen,
            Wicked,
            Eye,
            Solid,
            Burst,
            Keen,
            Brutal,
            Solid,
            Keen,
            Brutal,
        ]
    };
    let mut drain = rotation.drain(..);

    rt.add_event(
        Event::Cast(CastEvent {
            action: drain.next().unwrap(),
            source: player,
            targets: vec![boss],
        }),
        0,
    );
    rt.add_event(Event::DotTrigger, 300);

    while let Some((delta, event)) = rt.advance() {
        jobstate.advance(delta);
        println!("------------------------------------------");
        println!("{:?}", event);
        println!("{:?}", jobstate);
        println!("{:?}", rt.get_actor(player).unwrap());
        println!("{:?}", rt.get_actor(boss).unwrap());
        match event {
            Event::Cast(event) => {
                let mut evthandle = GnbEventProxy {
                    rt: &mut rt,
                    targets: &event.targets,
                    source: event.source,
                    stats: &pstats,
                    dots: &mut dots,
                };
                match jobstate.action_cast(event.action, pstats.sks_mod(), &mut evthandle) {
                    Err(v) => panic!("{:?}", v),
                    Ok(()) => (),
                }
                let ac = if let Some(v) = drain.next() {
                    v
                } else {
                    continue;
                };
                let delay = match jobstate.cooldown.error::<()>(ac.gcd()) {
                    Ok(()) => 0,
                    Err(ActionError::AnimationLock(v)) | Err(ActionError::GlobalCooldown(v)) => v,
                    v => panic!("{:?}", v),
                };
                rt.add_event(
                    Event::Cast(CastEvent {
                        action: ac,
                        source: player,
                        targets: if ac == GnbAction::NoMercy { vec![player] } else { vec![boss] },
                    }),
                    delay,
                );
            }
            Event::Damage(event) => {
                let target = rt.get_actor_mut(event.target).unwrap();
                target.health -= event.damage.dmg;
            }
            Event::EffectApply(event) => {
                let target = rt.get_actor_mut(event.target).unwrap();
                target.effects.push(event.effect);
            }
            // This is really really really bad and I'll come up with a better solution
            // I need a 2-way map which is a pain to implement
            Event::EffectRemove(event) => {
                let target = rt.get_actor_mut(event.target).unwrap();
                let ind = target
                    .effects
                    .iter()
                    .enumerate()
                    .find(|(_, x)| x.effect == event.effect)
                    .map(|(i, _)| i)
                    .unwrap();
                target.effects.swap_remove(ind);
            }
            Event::DotTrigger => {
                let mut should_delete = Vec::new();
                for (k, v) in &dots {
                    let target = if let Some(target) = rt.get_actor_mut(k.target) {
                        target
                    } else {
                        should_delete.push(*k);
                        continue;
                    };
                    if target
                        .effects
                        .iter_mut()
                        .find(|v| v.effect == k.effect)
                        .is_none()
                    {
                        should_delete.push(*k);
                        continue;
                    }
                    let damage = DamageInstance {
                        dmg: v.stats.prebuff_dot_damage(
                            CDHHandle::Avg {
                                chance: v.stats.crit_chance,
                            },
                            CDHHandle::Avg {
                                chance: v.stats.dhit_chance,
                            },
                            10000,
                        ),
                        el: v.el,
                        ty: v.ty,
                    };
                    let damage = v
                        .source
                        .iter()
                        .filter_map(|x| Some((x, x.effect.damage.outgoing?)))
                        .chain(
                            v.target
                                .iter()
                                .filter_map(|x| Some((x, x.effect.damage.outgoing?))),
                        )
                        .fold(damage, |c, (x, f)| f(*x, c));
                    target.health -= damage.dmg;
                }
                for x in should_delete {
                    dots.remove(&x);
                }
                if !(dots.is_empty() && rt.events() == 0) {
                    rt.add_event(Event::DotTrigger, 300);
                }
            }
        }
        println!("Total Boss Damage: {}", 1000000 - rt.get_actor(boss).unwrap().health);
    }
}
