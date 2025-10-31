extern crate serde;
use rltk::{GameState, Rltk, Point};
use specs::prelude::*;
use specs::saveload::{SimpleMarker, SimpleMarkerAllocator};

mod components;
pub use components::*;
mod map;
pub use map::*;
mod player;
use player::*;
mod rect;
pub use rect::Rect;
mod visibility_system;
use visibility_system::VisibilitySystem;
mod monster_ai_system;
use monster_ai_system::MonsterAI;
mod map_indexing_system;
use map_indexing_system::MapIndexingSystem;
mod melee_combat_system;
use melee_combat_system::MeleeCombatSystem;
mod damage_system;
use damage_system::DamageSystem;
mod gui;
mod gamelog;
mod spawner;
mod inventory_system;
use inventory_system::{ ItemCollectionSystem, ItemUseSystem, ItemDropSystem, ItemRemoveSystem };
pub mod saveload_system;
pub mod random_table;
pub mod particle_system;
pub mod hunger_system;
pub mod rex_assets;



#[derive(PartialEq, Copy, Clone)]
pub enum RunState {
    AwaitingInput, // 等待玩家输入
    PreRun,        // 游戏初始化阶段（如首次加载地图）
    PlayerTurn,    // 玩家回合
    MonsterTurn,   // 怪物回合
    ShowInventory, // 显示物品栏界面
    ShowDropItem,  // 显示丢弃物品界面
    ShowTargeting { range : i32, item : Entity }, // 显示瞄准界面，带范围和物品信息
    MainMenu { menu_selection : gui::MainMenuSelection }, // 主菜单状态，包含当前选项
    SaveGame,      // 保存游戏状态
    NextLevel,     // 进入下一层地图
    ShowRemoveItem,// 显示卸下装备界面
    GameOver,      // 游戏结束状态
    MagicMapReveal { row : i32 } // 魔法地图揭示状态，记录当前揭示的行
}

pub struct State {
    pub ecs: World // ECS 世界容器，包含所有实体和组件
}


impl State {
    fn run_systems(&mut self) {
        // 👁️ 视野系统：更新每个实体的可见区域
        let mut vis = VisibilitySystem{};
        vis.run_now(&self.ecs);

        // 🤖 怪物 AI 系统：控制怪物行为
        let mut mob = MonsterAI{};
        mob.run_now(&self.ecs);

        // 🗺️ 地图索引系统：更新地图上每个 tile 的实体列表
        let mut mapindex = MapIndexingSystem{};
        mapindex.run_now(&self.ecs);

        // ⚔️ 近战战斗系统：处理攻击行为
        let mut melee = MeleeCombatSystem{};
        melee.run_now(&self.ecs);

        // 💥 伤害系统：应用伤害并更新生命值
        let mut damage = DamageSystem{};
        damage.run_now(&self.ecs);

        // 🎒 拾取物品系统：处理玩家或怪物拾取物品
        let mut pickup = ItemCollectionSystem{};
        pickup.run_now(&self.ecs);

        // 🧪 使用物品系统：处理物品使用效果
        let mut itemuse = ItemUseSystem{};
        itemuse.run_now(&self.ecs);

        // 🗑️ 丢弃物品系统：将物品从背包丢到地图上
        let mut drop_items = ItemDropSystem{};
        drop_items.run_now(&self.ecs);

        // 🧥 卸下物品系统：将装备移回背包
        let mut item_remove = ItemRemoveSystem{};
        item_remove.run_now(&self.ecs);

        // 🍽️ 饥饿系统：处理饥饿状态变化与伤害
        let mut hunger = hunger_system::HungerSystem{};
        hunger.run_now(&self.ecs);

        // ✨ 粒子生成系统：生成视觉粒子效果
        let mut particles = particle_system::ParticleSpawnSystem{};
        particles.run_now(&self.ecs);

        // 🧹 清理 ECS 中的变化（如删除实体、更新组件）
        self.ecs.maintain();
    }
}


impl GameState for State {
    fn tick(&mut self, ctx : &mut Rltk) {
        let mut newrunstate;
        {
            let runstate = self.ecs.fetch::<RunState>();
            newrunstate = *runstate;
        }

        ctx.cls();
        particle_system::cull_dead_particles(&mut self.ecs, ctx);

        match newrunstate {
            RunState::MainMenu{..} => {}
            RunState::GameOver{..} => {}
            _ => {
                draw_map(&self.ecs, ctx);
                let positions = self.ecs.read_storage::<Position>();
                let renderables = self.ecs.read_storage::<Renderable>();
                let map = self.ecs.fetch::<Map>();

                let mut data = (&positions, &renderables).join().collect::<Vec<_>>();
                data.sort_by(|&a, &b| b.1.render_order.cmp(&a.1.render_order) );
                for (pos, render) in data.iter() {
                    let idx = map.xy_idx(pos.x, pos.y);
                    if map.visible_tiles[idx] { ctx.set(pos.x, pos.y, render.fg, render.bg, render.glyph) }
                }
                gui::draw_ui(&self.ecs, ctx);
            }
        }

        match newrunstate {
            // 游戏初始化阶段：运行所有系统一次，然后进入等待输入状态
            RunState::PreRun => {
                self.run_systems();           // 执行所有游戏逻辑系统（AI、战斗、拾取等）
                self.ecs.maintain();          // 应用 ECS 的所有变更（如删除实体）
                newrunstate = RunState::AwaitingInput; // 切换到等待玩家输入状态
            }

            // 等待玩家输入：调用输入处理函数，决定下一状态
            RunState::AwaitingInput => {
                newrunstate = player_input(self, ctx); // 根据玩家按键决定状态（如移动、打开菜单等）
            }

            // 玩家回合：运行所有系统，然后判断是否进入特殊状态或怪物回合
            RunState::PlayerTurn => {
                self.run_systems();           // 玩家行为触发的系统（如攻击、使用物品）
                self.ecs.maintain();          // 应用变更
                match *self.ecs.fetch::<RunState>() {
                    // 如果玩家使用了魔法地图，进入地图揭示状态
                    RunState::MagicMapReveal{ .. } => newrunstate = RunState::MagicMapReveal{ row: 0 },
                    // 否则进入怪物回合
                    _ => newrunstate = RunState::MonsterTurn
                }
            }

            // 怪物回合：运行所有系统（怪物 AI 等），然后回到等待输入状态
            RunState::MonsterTurn => {
                self.run_systems();           // 怪物行为系统
                self.ecs.maintain();          // 应用变更
                newrunstate = RunState::AwaitingInput; // 回到玩家输入状态
            }

            // 显示物品栏界面：处理玩家选择的物品
            RunState::ShowInventory => {
                let result = gui::show_inventory(self, ctx); // 显示物品菜单并获取玩家选择结果
                match result.0 {
                    gui::ItemMenuResult::Cancel => newrunstate = RunState::AwaitingInput, // 玩家取消，回到输入状态
                    gui::ItemMenuResult::NoResponse => {} // 玩家未选择，保持当前状态
                    gui::ItemMenuResult::Selected => {
                        let item_entity = result.1.unwrap(); // 获取玩家选择的物品实体
                        let is_ranged = self.ecs.read_storage::<Ranged>(); // 检查物品是否为远程类型
                        let is_item_ranged = is_ranged.get(item_entity);
                        if let Some(is_item_ranged) = is_item_ranged {
                            // 如果是远程物品，进入瞄准状态
                            newrunstate = RunState::ShowTargeting{ range: is_item_ranged.range, item: item_entity };
                        } else {
                            // 否则直接使用物品（近战或非瞄准类）
                            let mut intent = self.ecs.write_storage::<WantsToUseItem>();
                            intent.insert(*self.ecs.fetch::<Entity>(), WantsToUseItem{ item: item_entity, target: None }).expect("Unable to insert intent");
                            newrunstate = RunState::PlayerTurn; // 使用物品后进入玩家回合处理
                        }
                    }
                }
            }

            // 显示丢弃物品界面：处理玩家丢弃物品的请求
            RunState::ShowDropItem => {
                let result = gui::drop_item_menu(self, ctx); // 显示丢弃菜单
                match result.0 {
                    gui::ItemMenuResult::Cancel => newrunstate = RunState::AwaitingInput, // 玩家取消
                    gui::ItemMenuResult::NoResponse => {} // 未选择
                    gui::ItemMenuResult::Selected => {
                        let item_entity = result.1.unwrap(); // 获取要丢弃的物品实体
                        let mut intent = self.ecs.write_storage::<WantsToDropItem>();
                        intent.insert(*self.ecs.fetch::<Entity>(), WantsToDropItem{ item: item_entity }).expect("Unable to insert intent");
                        newrunstate = RunState::PlayerTurn; // 丢弃请求后进入玩家回合处理
                    }
                }
            }

            // 显示卸下物品界面：处理玩家卸下装备的请求
            RunState::ShowRemoveItem => {
                let result = gui::remove_item_menu(self, ctx); // 显示卸下菜单
                match result.0 {
                    gui::ItemMenuResult::Cancel => newrunstate = RunState::AwaitingInput, // 玩家取消
                    gui::ItemMenuResult::NoResponse => {} // 未选择
                    gui::ItemMenuResult::Selected => {
                        let item_entity = result.1.unwrap(); // 获取要卸下的物品实体
                        let mut intent = self.ecs.write_storage::<WantsToRemoveItem>();
                        intent.insert(*self.ecs.fetch::<Entity>(), WantsToRemoveItem{ item: item_entity }).expect("Unable to insert intent");
                        newrunstate = RunState::PlayerTurn; // 卸下请求后进入玩家回合处理
                    }
                }
            }

            RunState::ShowTargeting{range, item} => {
                let result = gui::ranged_target(self, ctx, range);
                match result.0 {
                    gui::ItemMenuResult::Cancel => newrunstate = RunState::AwaitingInput,
                    gui::ItemMenuResult::NoResponse => {}
                    gui::ItemMenuResult::Selected => {
                        let mut intent = self.ecs.write_storage::<WantsToUseItem>();
                        intent.insert(*self.ecs.fetch::<Entity>(), WantsToUseItem{ item, target: result.1 }).expect("Unable to insert intent");
                        newrunstate = RunState::PlayerTurn;
                    }
                }
            }
            // 🏠 主菜单界面
            RunState::MainMenu{ .. } => {
                // 显示主菜单并获取选择结果
                let result = gui::main_menu(self, ctx);
                match result {
                    // 玩家移动光标但未确认选择
                    gui::MainMenuResult::NoSelection{ selected } => {
                        newrunstate = RunState::MainMenu{ menu_selection: selected };
                    }
                    // 玩家确认选择
                    gui::MainMenuResult::Selected{ selected } => {
                        match selected {
                            // 开始新游戏
                            gui::MainMenuSelection::NewGame => newrunstate = RunState::PreRun,
                            // 加载游戏
                            gui::MainMenuSelection::LoadGame => {
                                saveload_system::load_game(&mut self.ecs); // 加载存档
                                newrunstate = RunState::AwaitingInput;     // 进入游戏
                                saveload_system::delete_save();            // 删除存档，实现永久死亡
                            }
                            // 退出游戏
                            gui::MainMenuSelection::Quit => {
                                ::std::process::exit(0); // 终止程序
                            }
                        }
                    }
                }
            }

            // ☠️ 游戏结束界面
            RunState::GameOver => {
                // 显示游戏结束菜单
                let result = gui::game_over(ctx);
                match result {
                    // 玩家未选择任何选项
                    gui::GameOverResult::NoSelection => {}
                    // 玩家选择返回主菜单
                    gui::GameOverResult::QuitToMenu => {
                        self.game_over_cleanup(); // 清理游戏状态（如删除实体）
                        newrunstate = RunState::MainMenu{ menu_selection: gui::MainMenuSelection::NewGame };
                    }
                }
            }

            // 💾 保存游戏状态
            RunState::SaveGame => {
                saveload_system::save_game(&mut self.ecs); // 保存游戏到文件
                // 返回主菜单并默认选中“加载游戏”
                newrunstate = RunState::MainMenu{ menu_selection : gui::MainMenuSelection::LoadGame };
            }

            // ⬇️ 进入下一层地图
            RunState::NextLevel => {
                self.goto_next_level();       // 切换地图层级并重置状态
                newrunstate = RunState::PreRun; // 重新初始化新层
            }
            
            RunState::MagicMapReveal{row} => {
                let mut map = self.ecs.fetch_mut::<Map>();
                for x in 0..MAPWIDTH {
                    let idx = map.xy_idx(x as i32,row);
                    map.revealed_tiles[idx] = true;
                }
                if row as usize == MAPHEIGHT-1 {
                    newrunstate = RunState::MonsterTurn;
                } else {
                    newrunstate = RunState::MagicMapReveal{ row: row+1 };
                }
            }
        }

        {
            let mut runwriter = self.ecs.write_resource::<RunState>();
            *runwriter = newrunstate;
        }
        damage_system::delete_the_dead(&mut self.ecs);
    }
}

impl State {
/// 当玩家进入下一层地图时，返回需要删除的实体列表
fn entities_to_remove_on_level_change(&mut self) -> Vec<Entity> {
    // 获取 ECS 中所有实体
    let entities = self.ecs.entities();

    // 获取玩家组件存储
    let player = self.ecs.read_storage::<Player>();
    // 获取背包组件存储
    let backpack = self.ecs.read_storage::<InBackpack>();
    // 获取玩家实体引用
    let player_entity = self.ecs.fetch::<Entity>();
    // 获取装备组件存储
    let equipped = self.ecs.read_storage::<Equipped>();

    // 初始化待删除实体列表
    let mut to_delete : Vec<Entity> = Vec::new();

    // 遍历所有实体
    for entity in entities.join() {
        let mut should_delete = true; // 默认标记为需要删除

        // 👤 如果是玩家实体，则不删除
        let p = player.get(entity);
        if let Some(_p) = p {
            should_delete = false;
        }

        // 🎒 如果是玩家背包中的物品，则不删除
        let bp = backpack.get(entity);
        if let Some(bp) = bp {
            if bp.owner == *player_entity {
                should_delete = false;
            }
        }

        // 🧥 如果是玩家已装备的物品，则不删除
        let eq = equipped.get(entity);
        if let Some(eq) = eq {
            if eq.owner == *player_entity {
                should_delete = false;
            }
        }

        // ✅ 如果仍然标记为需要删除，则加入删除列表
        if should_delete {
            to_delete.push(entity);
        }
    }

    // 返回所有待删除的实体
        to_delete
    }

    fn goto_next_level(&mut self) {
        // 🧹 删除当前层中不需要保留的实体（如怪物、掉落物等）
        let to_delete = self.entities_to_remove_on_level_change();
        for target in to_delete {
            self.ecs.delete_entity(target).expect("Unable to delete entity");
        }
    
        // 🗺️ 构建新地图并替换旧地图
        let worldmap;
        let current_depth;
        {
            let mut worldmap_resource = self.ecs.write_resource::<Map>();
            current_depth = worldmap_resource.depth; // 记录当前层数
            *worldmap_resource = Map::new_map_rooms_and_corridors(current_depth + 1); // 创建新地图（下一层）
            worldmap = worldmap_resource.clone(); // 克隆地图用于后续使用
        }
    
        // 👾 在新地图的房间中生成怪物（跳过第一个房间，保留给玩家）
        for room in worldmap.rooms.iter().skip(1) {
            spawner::spawn_room(&mut self.ecs, room, current_depth + 1);
        }
    
        // 🚶 将玩家放置在新地图的起始房间中心
        let (player_x, player_y) = worldmap.rooms[0].center(); // 获取第一个房间的中心坐标
        let mut player_position = self.ecs.write_resource::<Point>();
        *player_position = Point::new(player_x, player_y); // 更新玩家位置资源
    
        // 更新玩家实体的 Position 组件
        let mut position_components = self.ecs.write_storage::<Position>();
        let player_entity = self.ecs.fetch::<Entity>();
        let player_pos_comp = position_components.get_mut(*player_entity);
        if let Some(player_pos_comp) = player_pos_comp {
            player_pos_comp.x = player_x;
            player_pos_comp.y = player_y;
        }
    
        // 👁️ 标记玩家视野为“脏”，以便重新计算可见区域
        let mut viewshed_components = self.ecs.write_storage::<Viewshed>();
        let vs = viewshed_components.get_mut(*player_entity);
        if let Some(vs) = vs {
            vs.dirty = true;
        }
    
        // 📜 添加日志提示玩家进入新层并获得部分治疗
        let mut gamelog = self.ecs.fetch_mut::<gamelog::GameLog>();
        gamelog.entries.push("You descend to the next level, and take a moment to heal.".to_string());
    
        // ❤️ 给玩家恢复至少一半的生命值（如果当前生命值低于一半）
        let mut player_health_store = self.ecs.write_storage::<CombatStats>();
        let player_health = player_health_store.get_mut(*player_entity);
        if let Some(player_health) = player_health {
            player_health.hp = i32::max(player_health.hp, player_health.max_hp / 2);
        }
    }
    

    fn game_over_cleanup(&mut self) {
        // Delete everything
        let mut to_delete = Vec::new();
        for e in self.ecs.entities().join() {
            to_delete.push(e);
        }
        for del in to_delete.iter() {
            self.ecs.delete_entity(*del).expect("Deletion failed");
        }

        // Build a new map and place the player
        let worldmap;
        {
            let mut worldmap_resource = self.ecs.write_resource::<Map>();
            *worldmap_resource = Map::new_map_rooms_and_corridors(1);
            worldmap = worldmap_resource.clone();
        }

        // Spawn bad guys
        for room in worldmap.rooms.iter().skip(1) {
            spawner::spawn_room(&mut self.ecs, room, 1);
        }

        // Place the player and update resources
        let (player_x, player_y) = worldmap.rooms[0].center();
        let player_entity = spawner::player(&mut self.ecs, player_x, player_y);
        let mut player_position = self.ecs.write_resource::<Point>();
        *player_position = Point::new(player_x, player_y);
        let mut position_components = self.ecs.write_storage::<Position>();
        let mut player_entity_writer = self.ecs.write_resource::<Entity>();
        *player_entity_writer = player_entity;
        let player_pos_comp = position_components.get_mut(player_entity);
        if let Some(player_pos_comp) = player_pos_comp {
            player_pos_comp.x = player_x;
            player_pos_comp.y = player_y;
        }

        // Mark the player's visibility as dirty
        let mut viewshed_components = self.ecs.write_storage::<Viewshed>();
        let vs = viewshed_components.get_mut(player_entity);
        if let Some(vs) = vs {
            vs.dirty = true;
        }
    }
}

fn main() -> rltk::BError {
    use rltk::RltkBuilder;
    let mut context = RltkBuilder::simple80x50()
        .with_title("Roguelike Tutorial")
        .build()?;
    context.with_post_scanlines(true);
    let mut gs = State {
        ecs: World::new()
    };
    gs.ecs.register::<Position>();
    gs.ecs.register::<Renderable>();
    gs.ecs.register::<Player>();
    gs.ecs.register::<Viewshed>();
    gs.ecs.register::<Monster>();
    gs.ecs.register::<Name>();
    gs.ecs.register::<BlocksTile>();
    gs.ecs.register::<CombatStats>();
    gs.ecs.register::<WantsToMelee>();
    gs.ecs.register::<SufferDamage>();
    gs.ecs.register::<Item>();
    gs.ecs.register::<ProvidesHealing>();
    gs.ecs.register::<InflictsDamage>();
    gs.ecs.register::<AreaOfEffect>();
    gs.ecs.register::<Consumable>();
    gs.ecs.register::<Ranged>();
    gs.ecs.register::<InBackpack>();
    gs.ecs.register::<WantsToPickupItem>();
    gs.ecs.register::<WantsToUseItem>();
    gs.ecs.register::<WantsToDropItem>();
    gs.ecs.register::<Confusion>();
    gs.ecs.register::<SimpleMarker<SerializeMe>>();
    gs.ecs.register::<SerializationHelper>();
    gs.ecs.register::<Equippable>();
    gs.ecs.register::<Equipped>();
    gs.ecs.register::<MeleePowerBonus>();
    gs.ecs.register::<DefenseBonus>();
    gs.ecs.register::<WantsToRemoveItem>();
    gs.ecs.register::<ParticleLifetime>();
    gs.ecs.register::<HungerClock>();
    gs.ecs.register::<ProvidesFood>();
    gs.ecs.register::<MagicMapper>();

    gs.ecs.insert(SimpleMarkerAllocator::<SerializeMe>::new());

    let map : Map = Map::new_map_rooms_and_corridors(1);
    let (player_x, player_y) = map.rooms[0].center();

    let player_entity = spawner::player(&mut gs.ecs, player_x, player_y);

    gs.ecs.insert(rltk::RandomNumberGenerator::new());
    for room in map.rooms.iter().skip(1) {
        spawner::spawn_room(&mut gs.ecs, room, 1);
    }

    gs.ecs.insert(map);
    gs.ecs.insert(Point::new(player_x, player_y));
    gs.ecs.insert(player_entity);
    gs.ecs.insert(RunState::MainMenu{ menu_selection: gui::MainMenuSelection::NewGame });
    gs.ecs.insert(gamelog::GameLog{ entries : vec!["Welcome to Rusty Roguelike".to_string()] });
    gs.ecs.insert(particle_system::ParticleBuilder::new());
    gs.ecs.insert(rex_assets::RexAssets::new());

    rltk::main_loop(context, gs)
}
