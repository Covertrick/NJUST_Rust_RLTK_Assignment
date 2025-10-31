use specs::prelude::*;
use super::{WantsToPickupItem, Name, InBackpack, Position, gamelog::GameLog, WantsToUseItem,
    Consumable, ProvidesHealing, CombatStats, WantsToDropItem, InflictsDamage, Map, SufferDamage,
    AreaOfEffect, Confusion, Equippable, Equipped, WantsToRemoveItem, particle_system::ParticleBuilder,
    ProvidesFood, HungerClock, HungerState, MagicMapper, RunState};

    pub struct ItemCollectionSystem {}

    impl<'a> System<'a> for ItemCollectionSystem {
        // 定义系统所需的数据类型
        type SystemData = (
            ReadExpect<'a, Entity>,               // 玩家实体
            WriteExpect<'a, GameLog>,             // 游戏日志
            WriteStorage<'a, WantsToPickupItem>,  // 玩家想要拾取的物品
            WriteStorage<'a, Position>,           // 物品的位置
            ReadStorage<'a, Name>,                // 物品名称
            WriteStorage<'a, InBackpack>          // 背包组件
        );
    
        fn run(&mut self, data : Self::SystemData) {
            let (player_entity, mut gamelog, mut wants_pickup, mut positions, names, mut backpack) = data;
    
            // 遍历所有拾取请求
            for pickup in wants_pickup.join() {
                // 移除物品的位置组件（表示它不再在地图上）
                positions.remove(pickup.item);
                // 将物品添加到拾取者的背包中
                backpack.insert(pickup.item, InBackpack{ owner: pickup.collected_by }).expect("Unable to insert backpack entry");
    
                // 如果是玩家拾取，记录日志
                if pickup.collected_by == *player_entity {
                    gamelog.entries.push(format!("You pick up the {}.", names.get(pickup.item).unwrap().name));
                }
            }
    
            // 清空拾取请求
            wants_pickup.clear();
        }
    }
    

pub struct ItemUseSystem {}

// 实现ItemUseSystem系统，它处理玩家使用物品的各种情况
impl<'a> System<'a> for ItemUseSystem {
    // 定义系统所需的数据组件
    // 使用#[allow(clippy::type_complexity)]来避免复杂的类型检查警告
    #[allow(clippy::type_complexity)]
    type SystemData = ( ReadExpect<'a, Entity>,
                        WriteExpect<'a, GameLog>,
                        WriteExpect<'a, Map>,
                        Entities<'a>,
                        WriteStorage<'a, WantsToUseItem>,
                        ReadStorage<'a, Name>,
                        ReadStorage<'a, Consumable>,
                        ReadStorage<'a, ProvidesHealing>,
                        ReadStorage<'a, InflictsDamage>,
                        WriteStorage<'a, CombatStats>,
                        WriteStorage<'a, SufferDamage>,
                        ReadStorage<'a, AreaOfEffect>,
                        WriteStorage<'a, Confusion>,
                        ReadStorage<'a, Equippable>,
                        WriteStorage<'a, Equipped>,
                        WriteStorage<'a, InBackpack>,
                        WriteExpect<'a, ParticleBuilder>,
                        ReadStorage<'a, Position>,
                        ReadStorage<'a, ProvidesFood>,
                        WriteStorage<'a, HungerClock>,
                        ReadStorage<'a, MagicMapper>,
                        WriteExpect<'a, RunState>
                      );

    #[allow(clippy::cognitive_complexity)]
    fn run(&mut self, data : Self::SystemData) {
        // 解构系统所需的数据
        let (
            player_entity,        // 玩家实体
            mut gamelog,          // 游戏日志
            map,                  // 地图资源
            entities,             // 所有实体集合
            mut wants_use,        // 使用物品请求组件
            names,                // 名称组件
            consumables,          // 可消耗组件
            healing,              // 治疗组件
            inflict_damage,       // 伤害组件
            mut combat_stats,     // 战斗属性组件
            mut suffer_damage,    // 承受伤害组件
            aoe,                  // 范围效果组件
            mut confused,         // 混乱状态组件
            equippable,           // 可装备组件
            mut equipped,         // 已装备组件
            mut backpack,         // 背包组件
            mut particle_builder, // 粒子效果构建器
            positions,            // 位置组件
            provides_food,        // 食物组件
            mut hunger_clocks,    // 饥饿计时器组件
            magic_mapper,         // 魔法地图组件
            mut runstate          // 游戏运行状态
        ) = data;
    
        // 遍历所有使用物品的请求
        for (entity, useitem) in (&entities, &wants_use).join() {
            let mut used_item = true;
    
            // 🎯 目标选择逻辑
            let mut targets : Vec<Entity> = Vec::new();
            match useitem.target {
                None => {
                    // 没有指定目标，默认作用于玩家
                    targets.push(*player_entity);
                }
                Some(target) => {
                    // 检查是否是范围效果物品
                    let area_effect = aoe.get(useitem.item);
                    match area_effect {
                        None => {
                            // 单体目标：获取该位置上的所有实体
                            let idx = map.xy_idx(target.x, target.y);
                            for mob in map.tile_content[idx].iter() {
                                targets.push(*mob);
                            }
                        }
                        Some(area_effect) => {
                            // 范围目标：计算爆炸范围内的所有实体
                            let mut blast_tiles = rltk::field_of_view(target, area_effect.radius, &*map);
                            blast_tiles.retain(|p| p.x > 0 && p.x < map.width-1 && p.y > 0 && p.y < map.height-1 );
                            for tile_idx in blast_tiles.iter() {
                                let idx = map.xy_idx(tile_idx.x, tile_idx.y);
                                for mob in map.tile_content[idx].iter() {
                                    targets.push(*mob);
                                }
                                // 添加粒子效果
                                particle_builder.request(tile_idx.x, tile_idx.y, rltk::RGB::named(rltk::ORANGE), rltk::RGB::named(rltk::BLACK), rltk::to_cp437('░'), 200.0);
                            }
                        }
                    }
                }
            }
    
            // 🛡️ 装备物品逻辑
            if let Some(can_equip) = equippable.get(useitem.item) {
                let target_slot = can_equip.slot;
                let target = targets[0];
    
                // 卸下目标已有的同类装备
                let mut to_unequip : Vec<Entity> = Vec::new();
                for (item_entity, already_equipped, name) in (&entities, &equipped, &names).join() {
                    if already_equipped.owner == target && already_equipped.slot == target_slot {
                        to_unequip.push(item_entity);
                        if target == *player_entity {
                            gamelog.entries.push(format!("You unequip {}.", name.name));
                        }
                    }
                }
                for item in to_unequip.iter() {
                    equipped.remove(*item);
                    backpack.insert(*item, InBackpack{ owner: target }).expect("Unable to insert backpack entry");
                }
    
                // 装备新物品
                equipped.insert(useitem.item, Equipped{ owner: target, slot: target_slot }).expect("Unable to insert equipped component");
                backpack.remove(useitem.item);
                if target == *player_entity {
                    gamelog.entries.push(format!("You equip {}.", names.get(useitem.item).unwrap().name));
                }
            }
    
            // 🍗 食物逻辑
            if provides_food.get(useitem.item).is_some() {
                used_item = true;
                let target = targets[0];
                if let Some(hc) = hunger_clocks.get_mut(target) {
                    hc.state = HungerState::WellFed;
                    hc.duration = 20;
                    gamelog.entries.push(format!("You eat the {}.", names.get(useitem.item).unwrap().name));
                }
            }

            // If its a magic mapper...
            let is_mapper = magic_mapper.get(useitem.item);
            match is_mapper {
                None => {}
                Some(_) => {
                    used_item = true;
                    gamelog.entries.push("The map is revealed to you!".to_string());
                    *runstate = RunState::MagicMapReveal{ row : 0};
                }
            }

            // If it heals, apply the healing
            let item_heals = healing.get(useitem.item);
            match item_heals {
                None => {}
                Some(healer) => {
                    used_item = false;
                    for target in targets.iter() {
                        let stats = combat_stats.get_mut(*target);
                        if let Some(stats) = stats {
                            stats.hp = i32::min(stats.max_hp, stats.hp + healer.heal_amount);
                            if entity == *player_entity {
                                gamelog.entries.push(format!("You use the {}, healing {} hp.", names.get(useitem.item).unwrap().name, healer.heal_amount));
                            }
                            used_item = true;

                            let pos = positions.get(*target);
                            if let Some(pos) = pos {
                                particle_builder.request(pos.x, pos.y, rltk::RGB::named(rltk::GREEN), rltk::RGB::named(rltk::BLACK), rltk::to_cp437('♥'), 200.0);
                            }
                        }
                    }
                }
            }

            // If it inflicts damage, apply it to the target cell
            let item_damages = inflict_damage.get(useitem.item);
            match item_damages {
                None => {}
                Some(damage) => {
                    used_item = false;
                    for mob in targets.iter() {
                        SufferDamage::new_damage(&mut suffer_damage, *mob, damage.damage);
                        if entity == *player_entity {
                            let mob_name = names.get(*mob).unwrap();
                            let item_name = names.get(useitem.item).unwrap();
                            gamelog.entries.push(format!("You use {} on {}, inflicting {} hp.", item_name.name, mob_name.name, damage.damage));

                            let pos = positions.get(*mob);
                            if let Some(pos) = pos {
                                particle_builder.request(pos.x, pos.y, rltk::RGB::named(rltk::RED), rltk::RGB::named(rltk::BLACK), rltk::to_cp437('‼'), 200.0);
                            }
                        }

                        used_item = true;
                    }
                }
            }

            // Can it pass along confusion? Note the use of scopes to escape from the borrow checker!
            let mut add_confusion = Vec::new();
            {
                let causes_confusion = confused.get(useitem.item);
                match causes_confusion {
                    None => {}
                    Some(confusion) => {
                        used_item = false;
                        for mob in targets.iter() {
                            add_confusion.push((*mob, confusion.turns ));
                            if entity == *player_entity {
                                let mob_name = names.get(*mob).unwrap();
                                let item_name = names.get(useitem.item).unwrap();
                                gamelog.entries.push(format!("You use {} on {}, confusing them.", item_name.name, mob_name.name));

                                let pos = positions.get(*mob);
                                if let Some(pos) = pos {
                                    particle_builder.request(pos.x, pos.y, rltk::RGB::named(rltk::MAGENTA), rltk::RGB::named(rltk::BLACK), rltk::to_cp437('?'), 200.0);
                                }
                            }
                        }
                    }
                }
            }
            for mob in add_confusion.iter() {
                confused.insert(mob.0, Confusion{ turns: mob.1 }).expect("Unable to insert status");
            }

            // If its a consumable, we delete it on use
            if used_item {
                let consumable = consumables.get(useitem.item);
                match consumable {
                    None => {}
                    Some(_) => {
                        entities.delete(useitem.item).expect("Delete failed");
                    }
                }
            }
        }

        wants_use.clear();
    }
}

pub struct ItemDropSystem {}

impl<'a> System<'a> for ItemDropSystem {
    #[allow(clippy::type_complexity)]
    // 定义系统所需的数据类型
    type SystemData = (
        ReadExpect<'a, Entity>,               // 玩家实体
        WriteExpect<'a, GameLog>,             // 游戏日志
        Entities<'a>,                         // 所有实体集合
        WriteStorage<'a, WantsToDropItem>,    // 丢弃物品请求组件
        ReadStorage<'a, Name>,                // 名称组件（用于日志）
        WriteStorage<'a, Position>,           // 位置组件（用于放置物品）
        WriteStorage<'a, InBackpack>          // 背包组件（用于移除物品）
    );

    fn run(&mut self, data : Self::SystemData) {
        let (player_entity, mut gamelog, entities, mut wants_drop, names, mut positions, mut backpack) = data;

        // 遍历所有发起丢弃请求的实体
        for (entity, to_drop) in (&entities, &wants_drop).join() {
            // 获取丢弃者的位置
            let mut dropper_pos : Position = Position { x: 0, y: 0 };
            {
                let dropped_pos = positions.get(entity).unwrap();
                dropper_pos.x = dropped_pos.x;
                dropper_pos.y = dropped_pos.y;
            }

            // 将物品放置在丢弃者当前位置
            positions.insert(
                to_drop.item,
                Position { x: dropper_pos.x, y: dropper_pos.y }
            ).expect("Unable to insert position");

            // 从背包中移除该物品
            backpack.remove(to_drop.item);

            // 如果是玩家丢弃，记录日志
            if entity == *player_entity {
                gamelog.entries.push(format!(
                    "You drop the {}.",
                    names.get(to_drop.item).unwrap().name
                ));
            }
        }

        // 清空所有丢弃请求
        wants_drop.clear();
    }
}

pub struct ItemRemoveSystem {}

impl<'a> System<'a> for ItemRemoveSystem {
    #[allow(clippy::type_complexity)]
    // 定义系统所需的数据类型
    type SystemData = (
        Entities<'a>,                         // 所有实体集合
        WriteStorage<'a, WantsToRemoveItem>,  // 卸下物品请求组件
        WriteStorage<'a, Equipped>,           // 已装备组件
        WriteStorage<'a, InBackpack>          // 背包组件（卸下后放入背包）
    );

    fn run(&mut self, data : Self::SystemData) {
        let (entities, mut wants_remove, mut equipped, mut backpack) = data;

        // 遍历所有发起卸下请求的实体
        for (entity, to_remove) in (&entities, &wants_remove).join() {
            // 移除装备状态
            equipped.remove(to_remove.item);

            // 将物品放入背包
            backpack.insert(
                to_remove.item,
                InBackpack { owner: entity }
            ).expect("Unable to insert backpack");
        }

        // 清空所有卸下请求
        wants_remove.clear();
    }
}
