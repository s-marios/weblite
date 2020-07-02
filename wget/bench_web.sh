#!/bin/bash
http --ignore-stdin :8088/elapi/v1/devices/150.65.230.105:001101/echoCommands request:=@er5 &
http --ignore-stdin :8088/elapi/v1/devices/150.65.230.104:001101/echoCommands request:=@er5 &
wait
