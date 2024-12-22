rm service_a_socket
rm service_b_socket
rm service_c_socket

export RUST_LOG=trace

cargo build

cargo run --bin init &
sleep 1

cargo run --bin ping &

sleep 8

killall init
killall serviceA
killall serviceB
killall serviceC

rm service_a_socket
rm service_b_socket
rm service_c_socket