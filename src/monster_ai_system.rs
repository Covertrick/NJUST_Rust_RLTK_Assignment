use specs::prelude::*;
use super::{Viewshed, Monster, Map, Position, WantsToMelee, RunState, Confusion, particle_system::ParticleBuilder};
use rltk::{Point};

pub struct MonsterAI {}

impl<'a> System<'a> for MonsterAI {
    #[allow(clippy::type_complexity)]
    // 定义系统所需的数据类型
    type SystemData = (
        WriteExpect<'a, Map>,               // 地图资源（用于寻路和 tile 状态）
        ReadExpect<'a, Point>,              // 玩家位置（Point 类型）
        ReadExpect<'a, Entity>,             // 玩家实体引用
        ReadExpect<'a, RunState>,           // 当前游戏运行状态
        Entities<'a>,                       // 所有实体集合
        WriteStorage<'a, Viewshed>,         // 视野组件（用于判断是否看到玩家）
        ReadStorage<'a, Monster>,           // 怪物标记组件（筛选怪物）
        WriteStorage<'a, Position>,         // 位置组件（用于移动）
        WriteStorage<'a, WantsToMelee>,     // 攻击意图组件（用于发起攻击）
        WriteStorage<'a, Confusion>,        // 混乱状态组件（用于控制行为）
        WriteExpect<'a, ParticleBuilder>    // 粒子效果构建器（用于显示混乱效果）
    );


    fn run(&mut self, data : Self::SystemData) {
        // 解构所有资源和组件
        let (
            mut map, player_pos, player_entity, runstate, entities,
            mut viewshed, monster, mut position,
            mut wants_to_melee, mut confused, mut particle_builder
            ) = data;
        
        // 只在怪物回合运行
        if *runstate != RunState::MonsterTurn { return; }
                        

        for (entity, mut viewshed, _monster, mut pos) in (&entities, &mut viewshed, &monster, &mut position).join() {
            let mut can_act = true; // 默认可以行动
    
            let is_confused = confused.get_mut(entity);
            if let Some(i_am_confused) = is_confused {
                i_am_confused.turns -= 1; // 减少混乱持续时间
                if i_am_confused.turns < 1 {
                    confused.remove(entity); // 移除混乱状态
                }
                can_act = false; // 本回合不能行动
    
                // 显示混乱粒子效果（紫色问号）
                particle_builder.request(
                    pos.x, pos.y,
                    rltk::RGB::named(rltk::MAGENTA),
                    rltk::RGB::named(rltk::BLACK),
                    rltk::to_cp437('?'),
                    200.0
                );
            }
    

            if can_act {
                // 计算怪物与玩家之间的距离
                let distance = rltk::DistanceAlg::Pythagoras.distance2d(
                    Point::new(pos.x, pos.y),
                    *player_pos
                );
    
                // 如果距离小于 1.5（相邻），发起近战攻击
                if distance < 1.5 {
                    wants_to_melee.insert(
                        entity,
                        WantsToMelee{ target: *player_entity }
                    ).expect("Unable to insert attack");
                }
                else if viewshed.visible_tiles.contains(&*player_pos) {
                    // 使用 A* 寻路算法计算路径
                    let path = rltk::a_star_search(
                        map.xy_idx(pos.x, pos.y),
                        map.xy_idx(player_pos.x, player_pos.y),
                        &*map
                    );
    
                    // 如果路径有效且有多步，则移动到下一步
                    if path.success && path.steps.len() > 1 {
                        // 取消当前位置阻挡
                        let mut idx = map.xy_idx(pos.x, pos.y);
                        map.blocked[idx] = false;
    
                        // 更新怪物位置为路径中的下一步
                        pos.x = path.steps[1] as i32 % map.width;
                        pos.y = path.steps[1] as i32 / map.width;
    
                        // 设置新位置为阻挡
                        idx = map.xy_idx(pos.x, pos.y);
                        map.blocked[idx] = true;
    
                        // 标记视野为“脏”，需要重新计算
                        viewshed.dirty = true;
                    }
                }
            }
        }
    }
}
