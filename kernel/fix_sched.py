import re

with open('src/sched.rs', 'r') as f:
    content = f.read()

methods = """
    fn idle_slot(&self) -> Option<usize> {
        for slot in 0..self.thread_count {
            if self.threads[slot].id == self.idle_thread_id { return Some(slot); }
        }
        None
    }

    fn initial_context_pair(&mut self) -> Result<(*mut arch::x86_64::TaskContext, *const arch::x86_64::TaskContext), SchedulerError> {
        let bootstrap_slot = self.bootstrap_slot().ok_or(SchedulerError::MissingIdleThread)?;
        self.threads[bootstrap_slot].state = ThreadState::Exited;

        let target_slot = self.first_runnable_user_slot()
            .or_else(|| self.idle_slot())
            .ok_or(SchedulerError::MissingIdleThread)?;
        self.activate_thread_slot(target_slot)?;
        self.prepare_resume_slot(target_slot);

        let (left, right) = self.threads.split_at_mut(target_slot.max(bootstrap_slot));
        if bootstrap_slot < target_slot {
            let current = &mut left[bootstrap_slot].context as *mut arch::x86_64::TaskContext;
            let next = &right[0].context as *const arch::x86_64::TaskContext;
            Ok((current, next))
        } else {
            let next = &left[target_slot].context as *const arch::x86_64::TaskContext;
            let current = &mut right[0].context as *mut arch::x86_64::TaskContext;
            Ok((current, next))
        }
    }

    fn context_switch_pair(&mut self) -> Option<(*mut arch::x86_64::TaskContext, *const arch::x86_64::TaskContext, DispatchDecision)> {
        let current_slot = self.current_slot?;
        
        let mut next_slot_opt = None;
        if self.runqueue_bitmap != 0 {
            let best_p = self.runqueue_bitmap.trailing_zeros() as usize;
            if self.runqueue_depths[best_p] > 0 {
                next_slot_opt = Some(self.runqueues[best_p][0]);
            }
        }
        let next_slot = next_slot_opt?;
        if next_slot == current_slot { return None; }

        if self.threads[current_slot].state == ThreadState::Running {
            self.threads[current_slot].state = ThreadState::Runnable;
            let _ = self.enqueue(current_slot);
        }
        
        let next_slot = self.dequeue(self.priority_for_thread(next_slot))?;

        self.current_slot = Some(next_slot);
        self.threads[next_slot].state = ThreadState::Running;
        self.threads[next_slot].dispatch_count = self.threads[next_slot].dispatch_count.saturating_add(1);
        self.context_switches = self.context_switches.saturating_add(1);
        self.prepare_resume_slot(next_slot);
        let dispatch = self.dispatch_snapshot(next_slot);

        let (left, right) = self.threads.split_at_mut(current_slot.max(next_slot));
        if current_slot < next_slot {
            let current = &mut left[current_slot].context as *mut arch::x86_64::TaskContext;
            let next = &right[0].context as *const arch::x86_64::TaskContext;
            Some((current, next, dispatch))
        } else {
            let next = &left[next_slot].context as *const arch::x86_64::TaskContext;
            let current = &mut right[0].context as *mut arch::x86_64::TaskContext;
            Some((current, next, dispatch))
        }
    }
}
"""

content = content.replace("}\n\n#[derive(Copy, Clone)]\nstruct DispatchDecision", methods + "\n#[derive(Copy, Clone)]\nstruct DispatchDecision")

with open('src/sched.rs', 'w') as f:
    f.write(content)

