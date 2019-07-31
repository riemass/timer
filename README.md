# Timer
Simple timer implemented in the rust programming language.
The timer manages it's own thread, and communicates by pushing
timed tasks into a BinaryHeap. 
The remote thread is sleeping on a Condvar until work arrives.
No external dependencies, except for std.
