./RenderPipelineShaders/tools/rps_hlslc/linux-x64/bin/rps-hlslc $1 && \
gcc $(basename $1).g.c -include stdint.h  -include RenderPipelineShaders/src/runtime/common/rps_rpsl_host_dll.c -c -I RenderPipelineShaders/include/ -o $(basename $1 ".rpsl").o -fpic -DRPS_SHADER_GUEST=1 && \
gcc -shared $(basename $1 ".rpsl").o -o $(dirname $1)/$(basename $1 ".rpsl").so && \
rm $(basename $1 ".rpsl").o $(basename $1).g.c $(basename $1 ".rpsl").tmp.rps.ll
