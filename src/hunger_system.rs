// 引入 Specs ECS 框架
use specs::prelude::*;
// 引入相关组件和资源
use super::{HungerClock, RunState, HungerState, SufferDamage, gamelog::GameLog};

// 定义饥饿系统结构体
pub struct HungerSystem {}

impl<'a> System<'a> for HungerSystem {
    #[allow(clippy::type_complexity)]
    // 定义系统所需的数据类型
    type SystemData = (
        Entities<'a>,                      // 所有实体集合
        WriteStorage<'a, HungerClock>,     // 饥饿计时器组件
        ReadExpect<'a, Entity>,            // 玩家实体
        ReadExpect<'a, RunState>,          // 当前游戏运行状态
        WriteStorage<'a, SufferDamage>,    // 承受伤害组件
        WriteExpect<'a, GameLog>           // 游戏日志资源
    );

    // 系统运行逻辑
    fn run(&mut self, data : Self::SystemData) {
        // 解构所有资源和组件
        let (entities, mut hunger_clock, player_entity, runstate, mut inflict_damage, mut log) = data;

        // 遍历所有具有 HungerClock 的实体
        for (entity, mut clock) in (&entities, &mut hunger_clock).join() {
            let mut proceed = false;

            // 根据当前游戏状态判断是否处理该实体的饥饿逻辑
            match *runstate {
                RunState::PlayerTurn => {
                    // 玩家回合时只处理玩家实体
                    if entity == *player_entity {
                        proceed = true;
                    }
                }
                RunState::MonsterTurn => {
                    // 怪物回合不处理玩家
                    if entity != *player_entity {
                        proceed = false;
                    }
                }
                _ => proceed = false // 其他状态不处理
            }

            // 如果需要处理饥饿逻辑
            if proceed {
                // 减少饥饿计时器
                clock.duration -= 1;

                // 如果计时器归零，进入下一个饥饿阶段
                if clock.duration < 1 {
                    match clock.state {
                        HungerState::WellFed => {
                            // 从“吃得饱”变为“正常”
                            clock.state = HungerState::Normal;
                            clock.duration = 200;
                            if entity == *player_entity {
                                log.entries.push("You are no longer well fed.".to_string());
                            }
                        }
                        HungerState::Normal => {
                            // 从“正常”变为“饥饿”
                            clock.state = HungerState::Hungry;
                            clock.duration = 200;
                            if entity == *player_entity {
                                log.entries.push("You are hungry.".to_string());
                            }
                        }
                        HungerState::Hungry => {
                            // 从“饥饿”变为“挨饿”
                            clock.state = HungerState::Starving;
                            clock.duration = 200;
                            if entity == *player_entity {
                                log.entries.push("You are starving!".to_string());
                            }
                        }
                        HungerState::Starving => {
                            // 持续挨饿会造成伤害
                            if entity == *player_entity {
                                log.entries.push("Your hunger pangs are getting painful! You suffer 1 hp damage.".to_string());
                            }
                            // 添加 1 点伤害
                            SufferDamage::new_damage(&mut inflict_damage, entity, 1);
                        }
                    }
                }
            }
        }
    }
}
