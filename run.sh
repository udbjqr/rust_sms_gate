#!/bin/bash

pid=`ps -ef | grep './sms_gate' | grep -v grep | awk '{print $2}'`
kill $pid
cd /data/sms/sms_gate
sleep 1
nohup ./sms_gate 1>./log/com.out 2>./log/com.out &
cd log
sleep 0.5
tail -f sms_gate.log