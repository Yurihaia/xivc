use std::{collections::HashMap, convert::TryInto, env, fs};

use xivc::{
    job::gnb::{GnbActionCooldown, GnbEventHandler, GnbJobState},
    math::{
        ActionStat, HitTypeHandle, EotSnapshot, PlayerInfo, PlayerStats, SpeedStat, WeaponInfo, XivMath,
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
    effect: (ActorId, StatusEffect),
}

struct GnbEventProxy<'r> {
    rt: &'r mut Runtime<Event>,
    targets: &'r Vec<ActorId>,
    source: ActorId,
    stats: &'r XivMath,
    dots: &'r mut HashMap<DotEffect, DotSnapshot>,
    effect_delay: u32,
}

impl<'r> GnbEventHandler for GnbEventProxy<'r> {
    fn damage(&mut self, potency: u64) {
        for target_id in self.targets.iter().copied() {
            let source = self.rt.get_actor(self.source).unwrap();
            let target = self.rt.get_actor(target_id).unwrap();
            let stats = source.effects.values().chain(target.effects.values()).fold(
                self.stats.stats,
                |c, x| {
                    if let Some(f) = x.effect.stats {
                        print!("{} ", x.effect.name);
                        f(*x, c)
                    } else {
                        c
                    }
                },
            );
            let stats = XivMath::new(stats, self.stats.weapon, self.stats.info);
            let crit = source
                .effects
                .values()
                .filter_map(|x| Some((x, x.effect.crit.outgoing?)))
                .chain(
                    target
                        .effects
                        .values()
                        .filter_map(|x| Some((x, x.effect.crit.incoming?))),
                )
                .fold(stats.crt_chance(), |c, (x, f)| {
                    print!("{} ", x.effect.name);
                    f(*x, c)
                });
            let dhit = source
                .effects
                .values()
                .filter_map(|x| Some((x, x.effect.dhit.outgoing?)))
                .chain(
                    target
                        .effects
                        .values()
                        .filter_map(|x| Some((x, x.effect.dhit.outgoing?))),
                )
                .fold(stats.dh_chance(), |c, (x, f)| {
                    print!("{} ", x.effect.name);
                    f(*x, c)
                });
            let damage = DamageInstance {
                dmg: stats.prebuff_action_damage(
                    potency,
                    ActionStat::AttackPower,
                    100,
                    HitTypeHandle::Avg {
                        chance: crit as u16,
                    },
                    HitTypeHandle::Avg {
                        chance: dhit as u16,
                    },
                    10000,
                ),
                el: DamageElement::None,
                ty: DamageType::Slashing,
            };
            let damage = source
                .effects
                .values()
                .filter_map(|x| Some((x, x.effect.damage.outgoing?)))
                .chain(
                    target
                        .effects
                        .values()
                        .filter_map(|x| Some((x, x.effect.damage.incoming?))),
                )
                .fold(damage, |c, (x, f)| {
                    print!("{} ", x.effect.name);
                    f(*x, c)
                });
            self.rt.add_event(
                Event::Damage(DamageEvent {
                    damage,
                    source: self.source,
                    target: target_id,
                }),
                0,
            );
            print!("{}", damage.dmg);
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
                self.effect_delay,
            );
        }
    }

    fn dot_apply(&mut self, effect: EffectInstance, dot_potency: u64) {
        self.effect_delay = 0;
        self.effect_apply(effect);
        for target_id in self.targets.iter().copied() {
            let source = self.rt.get_actor(self.source).unwrap();
            let target = self.rt.get_actor(target_id).unwrap();
            let stats = source.effects.values().chain(target.effects.values()).fold(
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
                .values()
                .filter_map(|x| Some((x, x.effect.crit.outgoing?)))
                .chain(
                    target
                        .effects
                        .values()
                        .filter_map(|x| Some((x, x.effect.crit.incoming?))),
                )
                .fold(stats.crt_chance(), |c, (x, f)| f(*x, c));
            let dhit = source
                .effects
                .values()
                .filter_map(|x| Some((x, x.effect.dhit.outgoing?)))
                .chain(
                    target
                        .effects
                        .values()
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
                .values()
                .filter(|v| v.effect.damage.outgoing.is_some())
                .copied()
                .collect();
            let target_damage_debuffs = target
                .effects
                .values()
                .filter(|v| v.effect.damage.incoming.is_some())
                .copied()
                .collect();
            self.dots.insert(
                DotEffect {
                    effect: (self.source, effect.effect),
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
        "Chain" { crit { in = 100 } }
    );

    pub const LITANY: StatusEffect = status_effect!(
        "Litany" { crit { out = 100 } }
    );

    pub const DIV: StatusEffect = status_effect!(
        "Div" { damage { out = 106 / 100 }}
    );

    pub const TRICK: StatusEffect = status_effect!(
        "Trick" { damage { in = 105 / 100 }}
    );

    pub const TECH: StatusEffect = status_effect!(
        "Tech" { damage { out = 105 / 100 }}
    );

    pub const DEVOTION: StatusEffect = status_effect!(
        "Dev" { damage { out = 105 / 100 }}
    );

    // This implementation does not rely on the actual "stacks"
    // the effect has because actually reducing them would be a pain
    // so I just calculate the strength off of the time
    pub const EMBOLDEN: StatusEffect = status_effect!(
        "Embolden" { damage { out = |e, mut d| {
            // Ceiling division
            let m = 2 * ((e.time as u64 + 399) / 400) + 100;
            d.dmg = d.dmg * m / 100;
            d
        }}}
    );

    pub const BROTHERHOOD: StatusEffect = status_effect!(
        "BHood" { damage { out = 105 / 100 }}
    );

    pub const POT: StatusEffect = status_effect!(
        "Pot" {
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
        health: 3000000,
        effects: HashMap::new(),
        mirrors: Vec::new(),
    });

    let player = rt.add_actor(Actor {
        name: ":qwestgnb:",
        health: 200000,
        effects: HashMap::new(),
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

    // add_event(buffs::TECH, 2000, &[700, 700 + 12000], player);
    // add_event(buffs::EMBOLDEN, 2000, &[800, 800 + 12000], player);
    add_event(buffs::TRICK, 1500, &[800, 800 + 6000, 800 + 12000], boss);
    add_event(buffs::CHAIN, 1500, &[800, 800 + 12000], boss);
    add_event(buffs::DEVOTION, 1500, &[800], player);
    // add_event(buffs::DIV, 1500, &[1080, 1080 + 12000], player);

    // DoT Implementation
    // God this is so bad I really should just bite the bullet
    // and assume all damaging debuffs are just % multipliers
    let mut dots: HashMap<DotEffect, DotSnapshot> = HashMap::new();

    let mut rotation = {
        let out = Vec::new();
        let s = fs::read_to_string(env::args().next().unwrap()).unwrap();
        for l in s.lines() {
            for _ in l.split('\t').filter(|v| !v.is_empty()) {
                // idk I'll figure out some way. Probably through serde cause thats pretty cool
                // out.push(x.parse::<GnbAction>().unwrap());
            }
        }
        out.into_iter()
    };

    rt.add_event(
        Event::Cast(CastEvent {
            action: rotation.next().unwrap(),
            source: player,
            targets: vec![boss],
        }),
        0,
    );
    rt.add_event(Event::DotTrigger, 200);

    while let Some((delta, event)) = rt.advance() {
        jobstate.advance(delta);
        // println!("------------------------------------------");
        // println!("{:?}", event);
        // println!("{:?}", jobstate);
        // println!("{:?}", rt.get_actor(player).unwrap());
        // println!("{:?}", rt.get_actor(boss).unwrap());
        match event {
            Event::Cast(event) => {
                print!("{} {:?}: ", Centis(rt.global()), event.action);
                let mut evthandle = GnbEventProxy {
                    rt: &mut rt,
                    targets: &event.targets,
                    source: event.source,
                    stats: &pstats,
                    dots: &mut dots,
                    effect_delay: 60,
                };
                match jobstate.action_cast(event.action, pstats.sks_mod(), &mut evthandle) {
                    Err(v) => panic!("{:?}", v),
                    Ok(()) => (),
                }
                println!();
                let ac = if let Some(v) = rotation.next() {
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
                        targets: if ac == GnbAction::NoMercy {
                            vec![player]
                        } else {
                            vec![boss]
                        },
                    }),
                    if ac == GnbAction::NoMercy {
                        jobstate.cooldown.global() - 100
                    } else {
                        delay.max(if ac == GnbAction::Divide {
                            jobstate
                                .cooldown
                                .action(&GnbActionCooldown::Divide)
                                .saturating_sub(3000)
                        } else {
                            ac.try_into()
                                .map(|v: GnbActionCooldown| jobstate.cooldown.action(&v))
                                .unwrap_or(0)
                        })
                    },
                );
            }
            Event::Damage(event) => {
                let target = rt.get_actor_mut(event.target).unwrap();
                target.health -= event.damage.dmg;
            }
            Event::EffectApply(event) => {
                let target = rt.get_actor_mut(event.target).unwrap();
                target
                    .effects
                    .insert((event.source, event.effect.effect), event.effect);
            }
            Event::EffectRemove(event) => {
                let target = rt.get_actor_mut(event.target).unwrap();
                target.effects.remove(&event.effect);
            }
            Event::DotTrigger => {
                let mut total = 0;
                dots.retain(|k, v| {
                    let target = if let Some(target) = rt.get_actor_mut(k.target) {
                        target
                    } else {
                        return false;
                    };
                    if !target.effects.contains_key(&k.effect) {
                        return false;
                    }
                    let damage = DamageInstance {
                        dmg: v.stats.prebuff_dot_damage(
                            HitTypeHandle::Avg {
                                chance: v.stats.crit_chance,
                            },
                            HitTypeHandle::Avg {
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
                                .filter_map(|x| Some((x, x.effect.damage.incoming?))),
                        )
                        .fold(damage, |c, (x, f)| f(*x, c));
                    target.health -= damage.dmg;
                    total += damage.dmg;
                    true
                });
                if !(dots.is_empty() && rt.events() == 0) && rt.global() < 14700 {
                    rt.add_event(Event::DotTrigger, 300);
                }
                println!("{} DoT Tick {}", Centis(rt.global()), total);
            }
        }
    }
    println!(
        "Total Boss Damage: {}",
        3000000 - rt.get_actor(boss).unwrap().health
    );
}
