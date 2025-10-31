// 引入 Specs ECS 框架
use specs::prelude::*;
// 引入自定义模块中的类型
use super::{ Rltk, ParticleLifetime, Position, Renderable };
// 引入 RLTK 库中的颜色类型
use rltk::RGB;

/// 清除已过期的粒子实体
pub fn cull_dead_particles(ecs : &mut World, ctx : &Rltk) {
    let mut dead_particles : Vec<Entity> = Vec::new(); // 存储待删除的粒子实体
    {
        // 获取粒子生命周期组件的可写访问权限
        let mut particles = ecs.write_storage::<ParticleLifetime>();
        let entities = ecs.entities(); // 获取所有实体
        // 遍历所有具有 ParticleLifetime 的实体
        for (entity, mut particle) in (&entities, &mut particles).join() {
            // 减少粒子的剩余生命周期（单位：毫秒）
            particle.lifetime_ms -= ctx.frame_time_ms;
            // 如果生命周期小于 0，则标记为死亡
            if particle.lifetime_ms < 0.0 {
                dead_particles.push(entity);
            }
        }
    }
    // 删除所有已死亡的粒子实体
    for dead in dead_particles.iter() {
        ecs.delete_entity(*dead).expect("Particle will not die");
    }
}

/// 表示一个待生成的粒子请求
struct ParticleRequest {
    x: i32,                      // 粒子位置 X 坐标
    y: i32,                      // 粒子位置 Y 坐标
    fg: RGB,                    // 前景色
    bg: RGB,                    // 背景色
    glyph: rltk::FontCharType, // 显示字符
    lifetime: f32               // 生命周期（毫秒）
}

/// 粒子构建器，用于收集所有待生成的粒子请求
pub struct ParticleBuilder {
    requests : Vec<ParticleRequest> // 所有待生成的粒子列表
}

impl ParticleBuilder {
    /// 创建一个新的粒子构建器
    #[allow(clippy::new_without_default)]
    pub fn new() -> ParticleBuilder {
        ParticleBuilder{ requests : Vec::new() }
    }

    /// 添加一个粒子生成请求
    pub fn request(&mut self, x:i32, y:i32, fg: RGB, bg:RGB, glyph: rltk::FontCharType, lifetime: f32) {
        self.requests.push(
            ParticleRequest{
                x, y, fg, bg, glyph, lifetime
            }
        );
    }
}

/// 粒子生成系统，用于将请求转化为实际的粒子实体
pub struct ParticleSpawnSystem {}

impl<'a> System<'a> for ParticleSpawnSystem {
    #[allow(clippy::type_complexity)]
    // 定义系统所需的数据类型
    type SystemData = (
        Entities<'a>,                          // 实体集合
        WriteStorage<'a, Position>,            // 位置组件
        WriteStorage<'a, Renderable>,          // 渲染组件
        WriteStorage<'a, ParticleLifetime>,    // 生命周期组件
        WriteExpect<'a, ParticleBuilder>       // 粒子构建器资源
    );

    /// 系统运行逻辑：将所有请求转化为实体
    fn run(&mut self, data : Self::SystemData) {
        let (entities, mut positions, mut renderables, mut particles, mut particle_builder) = data;

        // 遍历所有待生成的粒子请求
        for new_particle in particle_builder.requests.iter() {
            let p = entities.create(); // 创建新实体
            // 添加位置组件
            positions.insert(p, Position{ x: new_particle.x, y: new_particle.y }).expect("Unable to insert position");
            // 添加渲染组件
            renderables.insert(p, Renderable{
                fg: new_particle.fg,
                bg: new_particle.bg,
                glyph: new_particle.glyph,
                render_order: 0
            }).expect("Unable to insert renderable");
            // 添加生命周期组件
            particles.insert(p, ParticleLifetime{ lifetime_ms: new_particle.lifetime }).expect("Unable to insert lifetime");
        }

        // 清空请求列表，避免重复生成
        particle_builder.requests.clear();
    }
}
